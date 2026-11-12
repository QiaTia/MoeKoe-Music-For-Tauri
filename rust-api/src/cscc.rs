//! 听歌时长 / 听歌等级（CSCC）上报模块。
//!
//! 对应 Node：`module/user_grade_info.js`、`module/user_listen_report.js`
//! （上游 `MakcRe/KuGouMusicApi#5d6c9f7`；v1.7.2 起随客户端内置，前端 `stores/listenTime.js` 调用）。
//!
//! 协议：
//! 1. `/user/grade/info` —— POST `http://userinfo.user.kugou.com/v2/get_grade_info`
//!    - 查询模式 p = RSA(publicLiteRasKey, JSON{clienttime,userid})（无填充、hex 大写）
//!    - 同步模式 p = RSA(publicLiteRasKey, JSON{token,md5(d_sec+diff_sec+y_type+m_type)})
//!    - key = md5(appid + appkey + clientver + clienttime)；该接口不生成 signature
//! 2. `/user/listen/report` —— CSCC 三连：`/v3/qrydid` 取 deviceid → `/v2/gen` 申请会话
//!    → `/v2/post` 加密上报（zlib deflate + AES-256-CBC，key=md5(uuid+clienttime+field2+serverstr)）
//!    - 会话按「账号 + 设备」缓存 30 分钟，并发建会话合并为一次
//!    - end 事件附加等级同步，响应形状 `{ status, data:{report, grade, playback_accepted, grade_synced} }`

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Map, Value};
use std::sync::Arc;

use crate::crypto::{crypto_md5, crypto_rsa_encrypt};
use crate::modules::{
    cookie_of, json_str, now_ms, now_sec, obj, q_num, q_num_or, q_str, str_map, sv, ModuleFuture,
    UseAxios,
};
use crate::request::{RequestOptions, Response};
use crate::sign::EncryptType;

// ---------------- 常量 ----------------

const CSCC_APP_ID: &str = "3116";
const CSCC_APP_KEY: &str = "LnT6xpN3khm36zse0QzvmgTZ3waWdRSA";
const CSCC_CLIENT_VER: &str = "10597";
const CSCC_BASE: &str = "http://d.kugou.com";
/// qrydid 阶段固定 appid 与签名盐
const CSCC_QRYDID_APPID: &str = "and02";
const CSCC_QRYDID_SALT: &str = "pbKC7zn{4U*ydo2M1Rir";
/// CSCC 会话缓存有效期（对应 Node 30 * 60 * 1000）
const CSCC_SESSION_TTL_SECS: u64 = 30 * 60;
/// 会话缓存上限（对应 Node 的 256 条淘汰）
const CSCC_SESSION_MAX: usize = 256;

#[derive(Clone)]
struct CsccSession {
    cookie: String,
    clienttime: String,
    serverstr: String,
    field2: String,
}

/// 建会话 / 上报共用的设备与账号上下文
#[derive(Clone)]
struct CsccCtx {
    uuid: String,
    mid: String,
    userid: String,
    token: String,
    device_model: String,
    system_version: String,
    screen_w: i64,
    screen_h: i64,
    local_ip: String,
    cookie: Map<String, Value>,
}

type CsccCell = Arc<tokio::sync::OnceCell<Result<Arc<CsccSession>, String>>>;

static CSCC_SESSIONS: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, CsccCell)>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// 对应 Node `clean()`：非空且不含分隔符/换行
fn cscc_clean(v: &str) -> bool {
    !v.is_empty() && !v.contains('&') && !v.contains('\t') && !v.contains('\r') && !v.contains('\n')
}

/// 对应 Node `integer()`：非负十进制整数（禁止前导零）
fn cscc_uint(v: &str) -> Option<i64> {
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if v.len() > 1 && v.starts_with('0') {
        return None;
    }
    v.parse::<i64>().ok()
}

/// 取参数：query 优先，其次 cookie（含别名）
fn cscc_param(q: &Value, cookie: &Map<String, Value>, key: &str, aliases: &[&str]) -> String {
    let v = q_str(q, key);
    if !v.is_empty() {
        return v;
    }
    for k in aliases {
        if let Some(cv) = cookie.get(*k) {
            let s = crate::jsval::js_to_string(cv);
            if !s.is_empty() {
                return s;
            }
        }
    }
    String::new()
}

/// 构造 CSCC 上下文（listen_report 用：uuid 去横线、强校验）
fn cscc_ctx(q: &Value) -> CsccCtx {
    let cookie = cookie_of(q);
    let uuid = cscc_param(q, &cookie, "uuid", &["uuid", "KUGOU_API_GUID"]).replace('-', "");
    let mid = cscc_param(q, &cookie, "mid", &["mid", "KUGOU_API_MID"]);
    let userid = cscc_param(q, &cookie, "userid", &["userid"]);
    let token = cscc_param(q, &cookie, "token", &["token"]);
    let device_model = {
        let a = q_str(q, "device_model");
        if !a.is_empty() {
            a
        } else {
            let b = q_str(q, "dev");
            if !b.is_empty() {
                b
            } else {
                let c = cookie
                    .get("KUGOU_API_DEV")
                    .map(crate::jsval::js_to_string)
                    .unwrap_or_default();
                if !c.is_empty() {
                    c
                } else {
                    std::env::var("KUGOU_API_DEV")
                        .ok()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "KuGouMusicApi".to_string())
                }
            }
        }
    };
    let system_version = {
        let v = q_str(q, "system_version");
        if v.is_empty() {
            "9".to_string()
        } else {
            v
        }
    };
    let local_ip = {
        let v = q_str(q, "local_ip");
        if v.is_empty() {
            "0.0.0.0".to_string()
        } else {
            v
        }
    };
    CsccCtx {
        uuid,
        mid,
        userid,
        token,
        device_model,
        system_version,
        screen_w: q_num_or(q, "screen_width", 1920),
        screen_h: q_num_or(q, "screen_height", 1080),
        local_ip,
        cookie,
    }
}

fn cscc_err(status: u16, msg: &str) -> Response {
    Response {
        status,
        body: json!({ "status": 0, "msg": msg }),
        cookie: Vec::new(),
        headers: std::collections::HashMap::new(),
    }
}

/// 校验设备参数（对应 Node 的 400 分支）
fn cscc_validate_device(ctx: &CsccCtx) -> Result<(), String> {
    if !cscc_clean(&ctx.device_model)
        || ctx.device_model.chars().count() > 128
        || ctx
            .device_model
            .chars()
            .any(|c| c == '\u{0}' || (c as u32) < 0x20 || c == '\u{7f}')
    {
        return Err("设备参数无效：需要有效机型、系统版本、正整数屏幕尺寸及 IPv4/IPv6 local_ip".into());
    }
    let ver_ok = !ctx.system_version.is_empty()
        && ctx.system_version.chars().count() <= 16
        && ctx
            .system_version
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if !ver_ok
        || !(1..=65535).contains(&ctx.screen_w)
        || !(1..=65535).contains(&ctx.screen_h)
        || ctx.local_ip.parse::<std::net::IpAddr>().is_err()
    {
        return Err("设备参数无效：需要有效机型、系统版本、正整数屏幕尺寸及 IPv4/IPv6 local_ip".into());
    }
    Ok(())
}

/// 校验账号参数
fn cscc_validate_account(ctx: &CsccCtx) -> Result<(), String> {
    let uuid_ok = ctx.uuid.len() == 32 && ctx.uuid.chars().all(|c| c.is_ascii_alphanumeric());
    let userid = cscc_uint(&ctx.userid).unwrap_or(0);
    if !uuid_ok || !cscc_clean(&ctx.mid) || userid <= 0 || ctx.token.is_empty() {
        return Err("需要 token、userid、mid 和 32 位字母数字 uuid（也可从 cookie 读取）".into());
    }
    Ok(())
}

/// 构造 /user/grade/info 请求（供 listen_report 的等级同步复用）
fn cscc_grade_options(q: &Value) -> RequestOptions {
    let cookie = cookie_of(q);
    let token = cscc_param(q, &cookie, "token", &["token"]);
    let userid = cscc_param(q, &cookie, "userid", &["userid"])
        .parse::<i64>()
        .unwrap_or(0);
    let mid = cscc_param(q, &cookie, "mid", &["mid", "KUGOU_API_MID"]);
    let uuid = {
        let v = cscc_param(q, &cookie, "uuid", &["uuid", "KUGOU_API_GUID"]);
        if v.is_empty() {
            "-".to_string()
        } else {
            v
        }
    };
    let dfid = {
        let v = cscc_param(q, &cookie, "dfid", &["dfid"]);
        if v.is_empty() {
            "-".to_string()
        } else {
            v
        }
    };
    let type_v = q_num_or(q, "type", 1);
    let clienttime = now_sec();
    let key = crypto_md5(&format!(
        "{}{}{}{}",
        CSCC_APP_ID, CSCC_APP_KEY, CSCC_CLIENT_VER, clienttime
    ));

    let has_sync = q.get("d_sec").map(|v| !v.is_null()).unwrap_or(false)
        && q.get("diff_sec").map(|v| !v.is_null()).unwrap_or(false);

    let p = if has_sync {
        let inner = crypto_md5(&format!(
            "{}{}{}{}",
            q_num(q, "d_sec"),
            q_num(q, "diff_sec"),
            q_num_or(q, "y_type", 0),
            q_num_or(q, "m_type", 0)
        ));
        crypto_rsa_encrypt(
            &json_str(&json!({ "token": token, "md5": inner })),
            Some(crate::crypto::PUBLIC_LITE_RSA_KEY),
        )
        .to_uppercase()
    } else {
        crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime": clienttime, "userid": userid })),
            Some(crate::crypto::PUBLIC_LITE_RSA_KEY),
        )
        .to_uppercase()
    };

    let mut data_map = obj(vec![
        ("mid", sv(mid)),
        ("type", Value::from(type_v)),
        ("uuid", sv(uuid)),
        ("userid", Value::from(userid)),
    ]);
    if has_sync {
        data_map.insert("d_sec".into(), Value::from(q_num(q, "d_sec")));
        data_map.insert("diff_sec".into(), Value::from(q_num(q, "diff_sec")));
        data_map.insert("y_type".into(), Value::from(q_num_or(q, "y_type", 0)));
        data_map.insert("m_type".into(), Value::from(q_num_or(q, "m_type", 0)));
    }
    data_map.insert("p".into(), sv(p));
    data_map.insert("appid".into(), sv(CSCC_APP_ID.to_string()));
    data_map.insert("clientver".into(), sv(CSCC_CLIENT_VER.to_string()));
    data_map.insert("clienttime".into(), Value::from(clienttime));
    data_map.insert("key".into(), sv(key));

    // KG-THash：每次请求随机 7 位 hex（对应 Math.random()*0xfffffff 转 hex 左补零）
    let thash = format!("{:07x}", rand::random::<u32>() & 0x0fff_ffff);
    RequestOptions {
        base_url: Some("http://userinfo.user.kugou.com".into()),
        url: "/v2/get_grade_info".into(),
        method: "POST".into(),
        data: Some(Value::Object(data_map)),
        params: obj(vec![("dfid", sv(dfid))]),
        encrypt_type: EncryptType::Android,
        not_signature: true,
        clear_default_params: true,
        clear_default_headers: true,
        headers: str_map(&[
            ("Content-Type", "text/plain; charset=ISO-8859-1"),
            ("User-Agent", "Android15-1070-10597-201-0-get_user_grade_info-wifi"),
            ("KG-THash", &thash),
            ("KG-Rec", "1"),
            ("KG-RC", "1"),
        ]),
        cookie,
        ..Default::default()
    }
}

/// 拍平参数：对齐 Node server.js `Object.assign({}, {cookie}, params, body)`（body 覆盖同名 query）。
/// dispatch 把 POST body 嵌在 `query.body`，CSCC 的 start/end 参数全部走 POST body。
fn cscc_flat_params(q: &Value) -> Value {
    let mut merged = match q {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    if let Some(Value::Object(body)) = q.get("body") {
        for (k, v) in body {
            merged.insert(k.clone(), v.clone());
        }
    }
    Value::Object(merged)
}

/// /user/grade/info —— 听歌等级与累计时长查询/同步
pub(crate) fn m_user_grade_info(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let query = cscc_flat_params(&query);
        use_axios(cscc_grade_options(&query)).await
    })
}

/// CSCC 单步请求（统一 clearDefaultParams/Headers + notSignature 语义）
fn cscc_step(
    ctx: &CsccCtx,
    url: &str,
    method: &str,
    params: Map<String, Value>,
    body: Option<Vec<u8>>,
    ua: Option<String>,
) -> RequestOptions {
    let mut headers = Map::new();
    if method == "POST" {
        headers.insert(
            "Content-Type".into(),
            Value::String("application/octet-stream".into()),
        );
    }
    if let Some(ua) = ua {
        headers.insert("KG-Rec".into(), Value::String("1".into()));
        headers.insert("User-Agent".into(), Value::String(ua));
    }
    RequestOptions {
        base_url: Some(CSCC_BASE.into()),
        url: url.into(),
        method: method.into(),
        params,
        data_binary: body,
        headers,
        encrypt_type: EncryptType::Android,
        not_signature: true,
        clear_default_params: true,
        clear_default_headers: true,
        cookie: ctx.cookie.clone(),
        ..Default::default()
    }
}

/// 申请/复用 CSCC 会话（/v3/qrydid → /v2/gen）
async fn cscc_establish(ctx: &CsccCtx, use_axios: &UseAxios) -> Result<CsccSession, String> {
    // 1. /v3/qrydid —— 换取 CSCC deviceid
    let t = now_sec().to_string();
    let data = json_str(&json!({
        "machine": ctx.device_model,
        "mid": ctx.mid,
        "uuid": ctx.uuid,
        "wh": [ctx.screen_w, ctx.screen_h],
    }));
    let sign = crypto_md5(&format!(
        "_t{}appid{}{}{}",
        t, CSCC_QRYDID_APPID, CSCC_QRYDID_SALT, data
    ));
    let opts = cscc_step(
        ctx,
        "/v3/qrydid",
        "POST",
        obj(vec![
            ("appid", sv(CSCC_QRYDID_APPID.to_string())),
            ("_t", sv(t)),
            ("sign", sv(sign)),
        ]),
        Some(data.into_bytes()),
        None,
    );
    let resp = use_axios(opts).await.map_err(|e| {
        e.body
            .get("msg")
            .map(crate::jsval::js_to_string)
            .unwrap_or_else(|| "CSCC request failed".to_string())
    })?;
    let deviceid = resp
        .body
        .get("data")
        .and_then(|d| d.get("deviceid"))
        .map(crate::jsval::js_to_string)
        .unwrap_or_default();
    if deviceid.is_empty() {
        return Err("CSCC 未返回 deviceid".into());
    }

    // 2. /v2/gen —— RSA-PKCS1(密文 base64) 申请会话，响应 AES-256-CBC 加密
    let field2 = hex::encode(rand::random::<[u8; 16]>());
    let seed = format!("{}\t{}\t{}\t{}", ctx.uuid, field2, now_ms(), deviceid);
    let s = crate::crypto::rsa_encrypt_pkcs1_b64(&seed, Some(crate::crypto::PUBLIC_LITE_RSA_KEY))?;
    let t2 = now_sec().to_string();
    let sign2 = crypto_md5(&format!("_t{}appid{}s{}{}", t2, CSCC_APP_ID, s, CSCC_APP_KEY));
    let ua = format!(
        "Android{}-1070-{}-18-0-CsccGen-wifi",
        ctx.system_version, CSCC_CLIENT_VER
    );
    let opts = cscc_step(
        ctx,
        "/v2/gen",
        "GET",
        obj(vec![
            ("s", sv(s)),
            ("appid", sv(CSCC_APP_ID.to_string())),
            ("_t", sv(t2)),
            ("sign", sv(sign2)),
        ]),
        None,
        Some(ua),
    );
    let resp = use_axios(opts).await.map_err(|e| {
        e.body
            .get("msg")
            .map(crate::jsval::js_to_string)
            .unwrap_or_else(|| "CSCC request failed".to_string())
    })?;
    let b64 = resp
        .body
        .get("data")
        .map(crate::jsval::js_to_string)
        .unwrap_or_default();
    let cipher = B64.decode(b64.trim()).unwrap_or_default();
    if cipher.is_empty() || ctx.uuid.len() < 16 {
        return Err("CSCC 会话响应无效".into());
    }
    let iv = ctx.uuid.as_bytes()[..16].to_vec();
    let plain = crate::crypto::aes_cbc_decrypt_bytes(field2.as_bytes(), &iv, &cipher);
    let session: Value = serde_json::from_slice(&plain).map_err(|_| "CSCC 会话响应无效")?;
    let cookie = session
        .get("cookie")
        .map(crate::jsval::js_to_string)
        .unwrap_or_default();
    let clienttime = session
        .get("clienttime")
        .map(crate::jsval::js_to_string)
        .unwrap_or_default();
    let serverstr = session
        .get("serverstr")
        .map(crate::jsval::js_to_string)
        .unwrap_or_default();
    if cookie.is_empty() || clienttime.is_empty() || serverstr.is_empty() {
        return Err("CSCC 会话响应无效".into());
    }
    Ok(CsccSession {
        cookie,
        clienttime,
        serverstr,
        field2,
    })
}

/// 取（或建）缓存会话：同一账号+设备 30 分钟内复用，并发请求合并
async fn cscc_session(ctx: &CsccCtx, use_axios: &UseAxios) -> Result<Arc<CsccSession>, String> {
    let cache_key = crypto_md5(&json_str(&json!([
        ctx.uuid,
        ctx.mid,
        ctx.userid,
        ctx.token,
        ctx.device_model,
        ctx.system_version,
        ctx.screen_w,
        ctx.screen_h,
    ])));

    let cell = {
        let mut map = match CSCC_SESSIONS.lock() {
            Ok(m) => m,
            Err(e) => e.into_inner(),
        };
        let now = std::time::Instant::now();
        map.retain(|_, (expires, _)| *expires > now);
        if let Some((_, cell)) = map.get(&cache_key) {
            cell.clone()
        } else {
            if map.len() >= CSCC_SESSION_MAX {
                if let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, (expires, _))| *expires)
                    .map(|(k, _)| k.clone())
                {
                    map.remove(&oldest);
                }
            }
            let cell: CsccCell = Arc::new(tokio::sync::OnceCell::new());
            map.insert(
                cache_key.clone(),
                (
                    now + std::time::Duration::from_secs(CSCC_SESSION_TTL_SECS),
                    cell.clone(),
                ),
            );
            cell
        }
    };

    let ctx_owned = ctx.clone();
    let ua_owned = use_axios.clone();
    let result = cell
        .get_or_init(|| async move {
            cscc_establish(&ctx_owned, &ua_owned)
                .await
                .map(Arc::new)
        })
        .await
        .clone();

    match result {
        Ok(s) => Ok(s),
        Err(e) => {
            // 建会话失败不缓存（对应 Node sessions.delete）
            if let Ok(mut map) = CSCC_SESSIONS.lock() {
                map.remove(&cache_key);
            }
            Err(e)
        }
    }
}

/// 构造 CSCC 上报事件字段（纯函数，便于单测）
fn cscc_build_fields(
    event: &str,
    song: &str,
    duration: i64,
    state: &str,
    ctx: &CsccCtx,
) -> Vec<String> {
    const FO3: &str = "3878,4320,4326,5747,5749,7229,7289,7863";
    let mut fields: Vec<String> = if event == "start" {
        vec![
            "type_id=20431".into(),
            "action=play".into(),
            format!("fo3={}", FO3),
            "spt=0".into(),
            "sty=手动".into(),
            format!("mixsongid={}", song),
            "source=46".into(),
            "ivar1=1".into(),
            "fo=/专辑播放页".into(),
        ]
    } else {
        vec![
            "type_id=4".into(),
            "action=play".into(),
            format!("fo3={}", FO3),
            format!("duration={}", duration),
            "svar3=0".into(),
            "svar2=0".into(),
            "fo=我的音乐/主态/自建歌单/RU".into(),
            "sty=手动".into(),
            format!("state={}", state),
            format!("mixsongid={}", song),
            "source=46".into(),
            "ivar3=0".into(),
            "type=1".into(),
            "fs=1.0".into(),
            "ivar1=1".into(),
        ]
    };
    fields.push(format!("mid={}", ctx.mid));
    fields.push(format!("uuid={}", ctx.uuid));
    fields.push("ss1=1".into());
    fields.push("ss2=1".into());
    fields.push(format!("sys={}", ctx.system_version));
    fields.push(format!("mod={}", ctx.device_model));
    fields.push("channelid=18".into());
    fields.push(format!("ip={}", ctx.local_ip));
    fields.push("net=1".into());
    fields.push(format!("ver={}", CSCC_CLIENT_VER));
    fields.push("gitversion=7aa8a76".into());
    fields.push(format!("time={}", now_ms()));
    fields.push(format!("userid={}", ctx.userid));
    // 对应 md5(crypto.randomUUID())
    let uuid4 = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand::random::<u32>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u64>() & 0xffff_ffff_ffff
    );
    fields.push(format!("ss3={}", crypto_md5(&uuid4)));
    fields
}

/// zlib deflate（对应 Node zlib.deflateSync，默认级别）
fn zlib_deflate(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data)
        .map_err(|e| format!("zlib deflate 失败: {e}"))?;
    enc.finish().map_err(|e| format!("zlib deflate 失败: {e}"))
}

/// CSCC `/v2/post` 上报签名（纯函数，便于单测）。
///
/// 对齐上游 `query.sign = md5(Buffer.concat([Buffer.from(sorted + APPKEY), encrypted]))`：
/// `sorted` 为 query 按键字典序拼接（`_t` < `appid` < `cookie` < `length`），
/// **APPKEY 必须夹在 sorted 与密文之间**，随后直接拼接二进制密文做字节级 md5。
fn cscc_post_sign(t: &str, cookie: &str, length: usize, cipher: &[u8]) -> String {
    let sorted = format!(
        "_t{}appid{}cookie{}length{}{}",
        t, CSCC_APP_ID, cookie, length, CSCC_APP_KEY
    );
    let mut src = sorted.into_bytes();
    src.extend_from_slice(cipher);
    crate::crypto::crypto_md5_bytes(&src)
}

/// /user/listen/report —— CSCC 播放事件上报（start / end，end 可带等级同步）
pub(crate) fn m_user_listen_report(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // start/end 参数由前端 POST body 传入（dispatch 嵌在 query.body），先拍平
        let query = cscc_flat_params(&query);
        let ctx = cscc_ctx(&query);
        if let Err(msg) = cscc_validate_device(&ctx) {
            return Err(cscc_err(400, &msg));
        }
        if let Err(msg) = cscc_validate_account(&ctx) {
            return Err(cscc_err(400, &msg));
        }

        let song = q_str(&query, "mixsongid");
        let event = q_str(&query, "event");
        let state = {
            let v = q_str(&query, "state");
            if v.is_empty() {
                "完整播放".to_string()
            } else {
                v
            }
        };
        if cscc_uint(&song).unwrap_or(0) <= 0 || !matches!(event.as_str(), "start" | "end") {
            return Err(cscc_err(400, "需要有效 mixsongid 和 event=start|end"));
        }
        let duration = cscc_uint(&q_str(&query, "duration")).unwrap_or(-1);
        if event == "end" && (duration < 0 || !cscc_clean(&state)) {
            return Err(cscc_err(
                400,
                "结束事件需要非负整数 duration（实际播放毫秒数）和有效 state",
            ));
        }
        // 是否携带等级同步参数
        let sync = query.get("d_sec").map(|v| !v.is_null()).unwrap_or(false)
            || query.get("diff_sec").map(|v| !v.is_null()).unwrap_or(false);
        if sync {
            let d_sec = cscc_uint(&q_str(&query, "d_sec")).unwrap_or(-1);
            let diff_sec = cscc_uint(&q_str(&query, "diff_sec")).unwrap_or(-1);
            if event != "end"
                || d_sec < 0
                || diff_sec < 0
                || diff_sec > (duration as f64 / 1000.0).ceil() as i64
            {
                return Err(cscc_err(
                    400,
                    "等级同步仅支持结束事件，需提供 d_sec、diff_sec（秒），增量不得超过本次播放时长",
                ));
            }
        }

        // ── 上报 ──
        let mut report_body = Value::Null;
        let mut playback_accepted = false;
        let report_result: Result<Response, String> = async {
            let session = cscc_session(&ctx, &use_axios).await?;
            let fields = cscc_build_fields(&event, &song, duration, &state, &ctx);
            let event_buffer = fields.join("&").into_bytes();
            let line1 = vec![
                "4".to_string(),
                "1".to_string(),
                ctx.uuid.clone(),
                "0".to_string(),
                CSCC_APP_ID.to_string(),
                CSCC_CLIENT_VER.to_string(),
                "18".to_string(),
                ctx.system_version.clone(),
                ctx.device_model.clone(),
                ctx.uuid.clone(),
                ctx.mid.clone(),
                "0".to_string(),
                "000000000000000000000000000000000000".to_string(),
            ]
            .join("\t");
            let line2 = vec![
                event_buffer.len().to_string(),
                "10048".to_string(),
                "0".to_string(),
                now_sec().to_string(),
                "1".to_string(),
                ctx.userid.clone(),
                "0".to_string(),
                "0".to_string(),
            ]
            .join("\t");
            let mut plain = format!("{}\r\n{}\r\n", line1, line2).into_bytes();
            plain.extend_from_slice(&zlib_deflate(&event_buffer)?);

            if ctx.uuid.len() < 16 {
                return Err("CSCC 会话响应无效".to_string());
            }
            // key = md5(uuid + clienttime + field2 + serverstr)：32 hex 字符 = 32 字节 → AES-256
            let key = crypto_md5(&format!(
                "{}{}{}{}",
                ctx.uuid, session.clienttime, session.field2, session.serverstr
            ));
            let iv = ctx.uuid.as_bytes()[..16].to_vec();
            let encrypted =
                crate::crypto::aes_cbc_encrypt_bytes(key.as_bytes(), &iv, &zlib_deflate(&plain)?);

            // sign = md5( (query 按键字典序拼接 + APPKEY) ++ 密文 )；键序：_t < appid < cookie < length
            let t = now_sec().to_string();
            let length = plain.len();
            let sign = cscc_post_sign(&t, &session.cookie, length, &encrypted);
            let length = length.to_string();

            let ua = format!(
                "Android{}-1070-{}-18-0-CsccPost-wifi",
                ctx.system_version, CSCC_CLIENT_VER
            );
            let opts = cscc_step(
                &ctx,
                "/v2/post",
                "POST",
                obj(vec![
                    ("cookie", sv(session.cookie.clone())),
                    ("length", sv(length)),
                    ("appid", sv(CSCC_APP_ID.to_string())),
                    ("_t", sv(t)),
                    ("sign", sv(sign)),
                ]),
                Some(encrypted),
                Some(ua),
            );
            let resp = use_axios(opts).await.map_err(|e| {
                let msg = e
                    .body
                    .get("msg")
                    .map(crate::jsval::js_to_string)
                    .unwrap_or_else(|| "CSCC request failed".to_string());
                msg
            })?;
            Ok(resp)
        }
        .await;

        match report_result {
            Ok(resp) => {
                report_body = resp.body;
                playback_accepted = true;
            }
            Err(_e) => {
                // 不重试：超时也可能已记账，避免重复上报。无同步时直接失败。
                if !sync {
                    return Err(cscc_err(502, "CSCC request failed"));
                }
            }
        }

        if !sync {
            return Ok(Response {
                status: 200,
                body: report_body,
                cookie: Vec::new(),
                headers: std::collections::HashMap::new(),
            });
        }

        // ── 等级同步（两步独立，不自动重试）──
        let (grade_body, grade_synced) = match use_axios(cscc_grade_options(&query)).await {
            Ok(resp) => {
                let ok = resp.body.get("status").and_then(|v| v.as_i64()) != Some(0)
                    && resp
                        .body
                        .get("error_code")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        == 0;
                (resp.body, ok)
            }
            Err(resp) => (resp.body, false),
        };

        let accepted = playback_accepted || grade_synced;
        let body = json!({
            "status": if accepted { 1 } else { 0 },
            "data": {
                "report": report_body,
                "grade": grade_body,
                "playback_accepted": playback_accepted,
                "grade_synced": grade_synced,
            }
        });
        let response = Response {
            status: if accepted { 200 } else { 502 },
            body,
            cookie: Vec::new(),
            headers: std::collections::HashMap::new(),
        };
        if accepted {
            Ok(response)
        } else {
            Err(response)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> CsccCtx {
        CsccCtx {
            uuid: "0123456789abcdef0123456789abcdef".into(),
            mid: "midvalue".into(),
            userid: "12345".into(),
            token: "tok".into(),
            device_model: "Pixel 7".into(),
            system_version: "14".into(),
            screen_w: 1920,
            screen_h: 1080,
            local_ip: "0.0.0.0".into(),
            cookie: Map::new(),
        }
    }

    #[test]
    fn test_cscc_uint_rules() {
        assert_eq!(cscc_uint("0"), Some(0));
        assert_eq!(cscc_uint("123"), Some(123));
        assert_eq!(cscc_uint("0123"), None, "禁止前导零（对应 Node integer()）");
        assert_eq!(cscc_uint(""), None);
        assert_eq!(cscc_uint("-1"), None);
        assert_eq!(cscc_uint("1.5"), None);
    }

    #[test]
    fn test_cscc_clean_rules() {
        assert!(cscc_clean("完整播放"));
        assert!(!cscc_clean(""));
        assert!(!cscc_clean("a&b"));
        assert!(!cscc_clean("a\tb"));
        assert!(!cscc_clean("a\r\nb"));
    }

    #[test]
    fn test_cscc_device_validation() {
        let mut c = ctx();
        assert!(cscc_validate_device(&c).is_ok());
        // 屏幕超界
        c.screen_w = 0;
        assert!(cscc_validate_device(&c).is_err());
        // 非法 IP
        let mut c2 = ctx();
        c2.local_ip = "not-an-ip".into();
        assert!(cscc_validate_device(&c2).is_err());
        // 机型含控制字符
        let mut c3 = ctx();
        c3.device_model = "bad\u{1}model".into();
        assert!(cscc_validate_device(&c3).is_err());
    }

    #[test]
    fn test_cscc_account_validation() {
        let c = ctx();
        assert!(cscc_validate_account(&c).is_ok());
        // uuid 非 32 位
        let mut c2 = ctx();
        c2.uuid = "short".into();
        assert!(cscc_validate_account(&c2).is_err());
        // userid 为 0
        let mut c3 = ctx();
        c3.userid = "0".into();
        assert!(cscc_validate_account(&c3).is_err());
        // token 缺失
        let mut c4 = ctx();
        c4.token = String::new();
        assert!(cscc_validate_account(&c4).is_err());
    }

    #[test]
    fn test_cscc_fields_start_and_end() {
        let c = ctx();
        let start = cscc_build_fields("start", "888", 0, "完整播放", &c);
        assert!(start.iter().any(|f| f == "type_id=20431"));
        assert!(start.iter().any(|f| f == "mixsongid=888"));
        assert!(start.iter().any(|f| f == "fo=/专辑播放页"));
        assert!(!start.iter().any(|f| f.starts_with("duration=")));
        // 公共字段齐全
        for key in ["mid=", "uuid=", "sys=", "mod=", "userid=", "ss3=", "ver="] {
            assert!(
                start.iter().any(|f| f.starts_with(key)),
                "缺少公共字段 {}",
                key
            );
        }

        let end = cscc_build_fields("end", "888", 123456, "完整播放", &c);
        assert!(end.iter().any(|f| f == "type_id=4"));
        assert!(end.iter().any(|f| f == "duration=123456"));
        assert!(end.iter().any(|f| f == "state=完整播放"));
        // 与 Node 一致：end 事件 15 个事件字段 + 14 个公共字段
        assert_eq!(end.len(), 29);
        assert_eq!(start.len(), 9 + 14);
    }

    #[test]
    fn test_grade_options_clear_defaults() {
        // /user/grade/info 用自定义 UA / Content-Type / KG-THash，
        // 必须同时关掉默认头与默认参数，否则会被 request.rs 的默认值覆盖。
        let q = json!({
            "token": "tok", "userid": "12345", "mid": "midvalue",
            "uuid": "0123456789abcdef0123456789abcdef",
        });
        let opts = cscc_grade_options(&q);
        assert!(opts.clear_default_headers);
        assert!(opts.clear_default_params);
        assert!(opts.not_signature);
        assert_eq!(opts.base_url.as_deref(), Some("http://userinfo.user.kugou.com"));
        assert_eq!(opts.url, "/v2/get_grade_info");
        assert_eq!(opts.method, "POST");
        assert!(opts.headers.contains_key("KG-THash"));
        assert!(opts.headers.contains_key("User-Agent"));
        assert!(opts.headers.contains_key("Content-Type"));
    }

    #[test]
    fn test_cscc_post_sign_includes_appkey() {
        // 对拍 Node：md5(Buffer.concat([Buffer.from(sorted + APPKEY), cipher]))
        //   sorted = "_t1700000000appid3116cookieabclength5"
        //   cipher = <01 02 03>
        // 漏掉 APPKEY 是真实踩过的坑：服务端校验必失败且静默丢弃上报。
        assert_eq!(
            cscc_post_sign("1700000000", "abc", 5, &[1, 2, 3]),
            "054e127c748c90fd4c25bb306b24b558"
        );
        // 键序必须是 _t < appid < cookie < length（字典序），且密文参与字节级 md5
        let with_key = format!(
            "_t1700000000appid{}cookieabclength5{}",
            CSCC_APP_ID, CSCC_APP_KEY
        );
        assert_eq!(
            cscc_post_sign("1700000000", "abc", 5, b""),
            crypto_md5(&with_key)
        );
    }

    #[test]
    fn test_zlib_deflate_roundtrip() {
        use std::io::Read;
        let src = b"type_id=4&action=play&mixsongid=888".to_vec();
        let packed = zlib_deflate(&src).unwrap();
        assert!(packed.len() > 2);
        let mut dec = flate2::read::ZlibDecoder::new(&packed[..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        assert_eq!(out, src);
    }

    #[test]
    fn test_cscc_cache_key_isolated_per_account_device() {
        // 账号/设备任一变化都必须换会话（对应 Node cacheKey 的组成）
        let a = ctx();
        let mut b = ctx();
        b.userid = "999".into();
        let key = |c: &CsccCtx| {
            crypto_md5(&json_str(&json!([
                c.uuid, c.mid, c.userid, c.token, c.device_model, c.system_version,
                c.screen_w, c.screen_h,
            ])))
        };
        assert_ne!(key(&a), key(&b));
        assert_eq!(key(&a), key(&ctx()));
    }
}
