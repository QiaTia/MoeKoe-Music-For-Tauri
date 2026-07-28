//! 接口模块层（Phase 1 样例）。
//!
//! 每个 Node 版 module/*.js 对应一个 async fn(query, use_axios) -> Result<Response, Response>。
//! query 是 server.js 注入的合并对象：{ ...query, body, cookie:{...}, ip }。
//! 注册表 REGISTRY 把 route(=文件名 `_`→`/`) 映射到处理函数，由 server 统一分发。

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Map, Value};
use std::sync::Arc;

use crate::config::{self, use_appid, use_clientver};
use crate::crypto::{
    crypto_aes_decrypt, crypto_aes_encrypt, crypto_md5, crypto_rsa_encrypt, crypto_sha1,
    playlist_aes_decrypt, playlist_aes_encrypt, rsa_encrypt2,
};
use crate::request::{RequestOptions, Response, ResponseType};
use crate::sign::{self, sign_cloud_key, sign_params, sign_params_key, EncryptType};
use crate::util::{calculate_mid, decode_lyrics, random_number, random_string};

/// 模块处理函数签名：Fn(query, useAxios) -> Future<Result<Response, Response>>
pub type ModuleFn = Arc<dyn Fn(Value, UseAxios) -> ModuleFuture + Send + Sync>;

// ---------------- query 取值辅助 ----------------

fn q_get<'a>(q: &'a Value, key: &str) -> Option<&'a Value> {
    q.get(key)
}

fn q_str(q: &Value, key: &str) -> String {
    match q.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// q_str 但空串时回退 default（对应 `params?.x || 'default'`）
fn q_str_or(q: &Value, key: &str, default: &str) -> String {
    let s = q_str(q, key);
    if s.is_empty() {
        default.to_string()
    } else {
        s
    }
}

fn q_num(q: &Value, key: &str) -> i64 {
    match q.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

/// q_num 但为 0/缺失时回退 default（对应 `params?.x || N`）
fn q_num_or(q: &Value, key: &str, default: i64) -> i64 {
    let n = q_num(q, key);
    if n == 0 {
        default
    } else {
        n
    }
}

fn q_bool(q: &Value, key: &str) -> bool {
    match q.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => s == "1" || s == "true",
        _ => false,
    }
}

fn q_cookie(q: &Value) -> &Value {
    q.get("cookie").unwrap_or(&Value::Null)
}

/// 构造一个空的 query 副本
#[allow(dead_code)]
fn q_clone(q: &Value) -> Value {
    q.clone()
}

// ---------------- cookie 取值辅助（对应 params?.cookie?.xxx） ----------------

/// 读取 cookie 里的字段（原始 Value）
fn ck_get<'a>(q: &'a Value, key: &str) -> Option<&'a Value> {
    q.get("cookie").and_then(|c| c.get(key))
}

/// cookie 字段转字符串（缺失/null -> ""）
fn ck_str(q: &Value, key: &str) -> String {
    match ck_get(q, key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// userid：`params?.userid || params?.cookie?.userid || 0` -> i64
fn userid_of(q: &Value) -> i64 {
    let a = q_num(q, "userid");
    if a != 0 {
        return a;
    }
    match ck_get(q, "userid") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

/// token：`params?.token || params?.cookie?.token || ''` -> String
fn token_of(q: &Value) -> String {
    let a = q_str(q, "token");
    if !a.is_empty() {
        return a;
    }
    ck_str(q, "token")
}

/// mid：`params?.cookie?.KUGOU_API_MID`（保留原始 Value，缺失 -> Null）
fn mid_of(q: &Value) -> Value {
    ck_get(q, "KUGOU_API_MID").cloned().unwrap_or(Value::Null)
}

/// dfid：`params?.cookie?.dfid || params?.dfid || '-'`
fn dfid_of(q: &Value) -> String {
    let c = ck_str(q, "dfid");
    if !c.is_empty() {
        return c;
    }
    q_str_or(q, "dfid", "-")
}

// ---------------- 时间辅助 ----------------

/// Date.now() —— 毫秒
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Math.floor(Date.now()/1000) —— 秒
fn now_sec() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// ---------------- Map/Value 构造辅助 ----------------

/// 从 (key, Value) 列表构造有序 Map
fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    m
}

/// 对应 `params?.x || N`：提供（req.query 恒为字符串）则原样字符串，缺省则数字 N。
/// 用于 POST data 体以精确复刻 JSON.stringify 的类型（字符串 vs 数字）。
fn s_or_num(q: &Value, key: &str, default: i64) -> Value {
    let s = q_str(q, key);
    if s.is_empty() {
        Value::from(default)
    } else {
        Value::String(s)
    }
}

/// 对应 `Number(params.x)`：转数字（NaN/缺省 -> 0）
fn n_of(q: &Value, key: &str) -> Value {
    Value::from(q_num(q, key))
}

/// 对应 `Number(params.x) || N`
fn n_or(q: &Value, key: &str, default: i64) -> Value {
    Value::from(q_num_or(q, key, default))
}

/// 字符串 -> Value::String
fn sv(s: String) -> Value {
    Value::String(s)
}

/// 自定义 android 风格签名（对应部分模块内联的 signatureAndroidParams 变体）：
/// sort(keys).map(k=> k + '=' + (object?JSON.stringify:`${v}`)).join(sep)
/// 返回拼接串（不含 md5），由调用方决定 md5/substring。
fn kv_sorted(map: &Map<String, Value>, sep: &str) -> String {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys.iter()
        .map(|k| {
            let v = &map[*k];
            let vs = match v {
                Value::Object(_) | Value::Array(_) | Value::Null => {
                    serde_json::to_string(v).unwrap_or_default()
                }
                _ => crate::jsval::js_to_string(v),
            };
            format!("{}={}", k, vs)
        })
        .collect::<Vec<_>>()
        .join(sep)
}

// ---------------- 样例模块 ----------------

/// /search —— 搜索
fn m_search(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let keyword = q_str(&query, "keywords");
        let page = if q_str(&query, "page").is_empty() {
            1i64
        } else {
            q_num(&query, "page")
        };
        let pagesize = if q_str(&query, "pagesize").is_empty() {
            30i64
        } else {
            q_num(&query, "pagesize")
        };
        let valid = ["special", "lyric", "song", "album", "author", "mv"];
        let typ = q_str(&query, "type");
        let typ = if valid.contains(&typ.as_str()) {
            typ
        } else {
            "song".to_string()
        };

        let mut data_map = Map::new();
        data_map.insert("albumhide".into(), Value::from(0));
        data_map.insert("iscorrection".into(), Value::from(1));
        data_map.insert("keyword".into(), Value::String(keyword));
        data_map.insert("nocollect".into(), Value::from(0));
        data_map.insert("page".into(), Value::from(page));
        data_map.insert("pagesize".into(), Value::from(pagesize));
        data_map.insert("platform".into(), Value::String("AndroidFilter".into()));

        let url = format!("/{}/search/{}", if typ == "song" { "v3" } else { "v1" }, typ);

        let opts = RequestOptions {
            url,
            method: "GET".into(),
            params: data_map,
            encrypt_type: EncryptType::Android,
            headers: str_map(&[("x-router", "complexsearch.kugou.com")]),
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /lyric —— 歌词获取（含 decode 分支）
fn m_lyric(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data_map = Map::new();
        data_map.insert("ver".into(), Value::from(1));
        let client = {
            let c = q_str(&query, "client");
            if c.is_empty() { "android".to_string() } else { c }
        };
        data_map.insert("client".into(), Value::String(client));
        data_map.insert("id".into(), query.get("id").cloned().unwrap_or(Value::Null));
        data_map.insert(
            "accesskey".into(),
            query.get("accesskey").cloned().unwrap_or(Value::Null),
        );
        let fmt = if q_str(&query, "fmt").is_empty() {
            "krc".to_string()
        } else {
            q_str(&query, "fmt")
        };
        data_map.insert("fmt".into(), Value::String(fmt.clone()));
        data_map.insert("charset".into(), Value::String("utf8".into()));

        let opts = RequestOptions {
            base_url: Some("https://lyrics.kugou.com".into()),
            url: "/download".into(),
            method: "GET".into(),
            params: data_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        if q_bool(&query, "decode") {
            if let Some(content) = res.body.get("content").and_then(|v| v.as_str()) {
                let decode_content = if fmt == "lrc"
                    || res.body.get("contenttype").and_then(|v| v.as_i64()) != Some(0)
                {
                    // base64 直接解码为 utf8 字符串
                    match B64.decode(content) {
                        Ok(b) => String::from_utf8_lossy(&b).to_string(),
                        Err(_) => content.to_string(),
                    }
                } else {
                    decode_lyrics(content)
                };
                if let Some(obj) = res.body.as_object_mut() {
                    obj.insert("decodeContent".into(), Value::String(decode_content));
                }
            }
        }
        Ok(res)
    })
}

/// /album/detail —— 专辑详情（POST + data）
fn m_album_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data = Map::new();
        let mut arr = Vec::new();
        let mut album = Map::new();
        album.insert(
            "album_id".into(),
            query.get("id").cloned().unwrap_or(Value::Null),
        );
        arr.push(Value::Object(album));
        data.insert("data".into(), Value::Array(arr));
        data.insert("is_buy".into(), Value::from(q_num(&query, "is_buy")));
        data.insert("fields".into(), Value::String("album_id,album_name,publish_date,sizable_cover,intro,language,is_publish,heat,type,quality,authors,exclusive,author_name,trans_param".into()));

        let opts = RequestOptions {
            url: "/kmr/v2/albums".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[
                ("x-router", "openapi.kugou.com"),
                ("kg-tid", "255"),
            ]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /song/url —— 获取音乐直链（encryptKey + notSign 拼写错误 -> 仍计算签名）
fn m_song_url(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let is_lite = config::is_lite();
        let magic = ["piano", "acappella", "subwoofer", "ancient", "dj", "surnay"];
        let quality_raw = q_str(&query, "quality");
        let quality = if magic.contains(&quality_raw.as_str()) {
            format!("magic_{}", quality_raw)
        } else {
            quality_raw
        };

        let page_id: i64 = if is_lite { 967177915 } else { 151369488 };
        let ppage_id = if is_lite {
            let p = q_str(&query, "ppage_id");
            if p.is_empty() {
                "356753938,823673182,967485191".to_string()
            } else {
                p
            }
        } else {
            "463467626,350369493,788954147".to_string()
        };

        let mut data_map = Map::new();
        data_map.insert(
            "album_id".into(),
            Value::from(q_num(&query, "album_id") as i64),
        );
        data_map.insert("area_code".into(), Value::from(1));
        let hash = q_str(&query, "hash").to_lowercase();
        data_map.insert("hash".into(), Value::String(hash));
        data_map.insert("ssa_flag".into(), Value::String("is_fromtrack".into()));
        data_map.insert("version".into(), Value::from(11430));
        data_map.insert("page_id".into(), Value::from(page_id));
        data_map.insert(
            "quality".into(),
            if quality.is_empty() {
                Value::from(128)
            } else {
                Value::String(quality)
            },
        );
        data_map.insert(
            "album_audio_id".into(),
            Value::from(q_num(&query, "album_audio_id") as i64),
        );
        data_map.insert("behavior".into(), Value::String("play".into()));
        data_map.insert("pid".into(), Value::from(if is_lite { 411 } else { 2 }));
        data_map.insert("cmd".into(), Value::from(26));
        data_map.insert("pidversion".into(), Value::from(3001));
        data_map.insert(
            "IsFreePart".into(),
            Value::from(if q_bool(&query, "free_part") { 1 } else { 0 }),
        );
        data_map.insert("ppage_id".into(), Value::String(ppage_id));
        data_map.insert("cdnBackup".into(), Value::from(1));
        data_map.insert("module".into(), Value::String(String::new()));
        data_map.insert("clientver".into(), Value::from(11430));

        // 复刻 bug：参数名是 notSign（拼写错误），request 层只识别 not_signature，
        // 因此签名【仍会】计算 —— 与 Node 行为完全一致。
        let mut cookie = cookie_of(&query);
        cookie.insert(
            "dfid".into(),
            Value::String(random_string(24).to_lowercase()),
        );

        let opts = RequestOptions {
            url: "/v5/url".into(),
            method: "GET".into(),
            params: data_map,
            encrypt_type: EncryptType::Android,
            headers: str_map(&[("x-router", "trackercdn.kugou.com")]),
            encrypt_key: true,
            not_signature: false, // 复刻 notSign 拼写错误（不生效）
            cookie,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /server/now —— 服务器时间（POST + data + params）
fn m_server_now(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = if q_str(&query, "userid").is_empty() {
            q_get(&query, "cookie")
                .and_then(|c| c.get("userid"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        } else {
            q_num(&query, "userid")
        };
        let token = if q_str(&query, "token").is_empty() {
            q_get(&query, "cookie")
                .and_then(|c| c.get("token"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            q_str(&query, "token")
        };

        let mut data = Map::new();
        data.insert("token".into(), Value::String(token));
        data.insert("userid".into(), Value::from(userid));
        let mut params = Map::new();
        params.insert("plat".into(), Value::from(3));

        let opts = RequestOptions {
            url: "/v1/server_now".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "usercenter.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/lyric —— 歌词搜索（clearDefaultParams + notSign）
fn m_search_lyric(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data_map = Map::new();
        data_map.insert(
            "album_audio_id".into(),
            Value::from(q_num(&query, "album_audio_id")),
        );
        data_map.insert("appid".into(), Value::from(config::use_appid()));
        data_map.insert("clientver".into(), Value::from(config::use_clientver()));
        data_map.insert("duration".into(), Value::from(0));
        data_map.insert(
            "hash".into(),
            Value::String(q_str(&query, "hash").to_lowercase()),
        );
        data_map.insert("keyword".into(), Value::String(q_str(&query, "keywords")));
        data_map.insert("lrctxt".into(), Value::from(1));
        let man = if q_get(&query, "man").is_some() {
            q_str(&query, "man")
        } else {
            "no".to_string()
        };
        data_map.insert("man".into(), Value::String(man));

        // Node 源码写的是 notSign:true，但 request 层只识别 notSignature，
        // 拼写错误导致不生效 —— 实际请求是【带签名】的。此处复刻真实行为：签名生效。
        let opts = RequestOptions {
            base_url: Some("https://lyrics.kugou.com".into()),
            url: "/v1/search".into(),
            method: "GET".into(),
            params: data_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            clear_default_params: true,
            not_signature: false, // 复刻 notSign 拼写错误（不生效）
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /login/qr/key —— 二维码 key（web 签名）
fn m_login_qr_key(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let is_web = q_str(&query, "type") == "web";
        let mut params = Map::new();
        params.insert("appid".into(), Value::from(if is_web { 1014 } else { 1001 }));
        params.insert("type".into(), Value::from(1));
        params.insert("plat".into(), Value::from(4));
        params.insert(
            "qrcode_txt".into(),
            Value::String(format!(
                "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid={}&",
                config::use_appid()
            )),
        );
        params.insert("srcappid".into(), Value::from(config::CONFIG.srcappid));

        let opts = RequestOptions {
            base_url: Some("https://login-user.kugou.com".into()),
            url: "/v2/qrcode".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Web,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /login/qr/create —— 本地生成二维码（不发起外部请求）
fn m_login_qr_create(query: Value, _use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let key = q_str(&query, "key");
        let url = format!(
            "https://h5.kugou.com/apps/loginQRCode/html/index.html?qrcode={}",
            key
        );
        // 仅当请求 qrimg 时才生成图片 base64（需要 image 编码）
        let base64 = if q_bool(&query, "qrimg") {
            match generate_qr_base64(&url) {
                Some(b) => b,
                None => String::new(),
            }
        } else {
            String::new()
        };

        let mut data = Map::new();
        data.insert("url".into(), Value::String(url));
        data.insert("base64".into(), Value::String(base64));
        // 对应 Node：moduleResponse.body = { code: 200, data: {...} }
        let mut body = Map::new();
        body.insert("code".into(), Value::from(200));
        body.insert("data".into(), Value::Object(data));

        Ok(Response {
            status: 200,
            body: Value::Object(body),
            cookie: Vec::new(),
            headers: std::collections::HashMap::new(),
        })
    })
}

// ================ 全量模块（批次 1：ai_recommend ~ comment_music_hotword） ================

/// /ai/recommend
fn m_ai_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = userid_of(&query);
        let clienttime = now_ms();
        let recommend_source: Vec<Value> = q_str(&query, "album_audio_id")
            .split(',')
            .map(|x| json!({ "ID": x.parse::<i64>().unwrap_or(0) }))
            .collect();
        let data = obj(vec![
            ("platform", sv("ios".into())),
            ("clientver", Value::from(use_clientver())),
            ("clienttime", Value::from(clienttime)),
            ("userid", Value::from(userid)),
            ("client_playlist", Value::Array(vec![])),
            ("source_type", Value::from(2)),
            ("playlist_ver", Value::from(2)),
            ("area_code", Value::from(1)),
            ("appid", Value::from(use_appid())),
            ("key", sv(sign_params_key(&clienttime.to_string(), None, None))),
            ("mid", mid_of(&query)),
            ("recommend_source", Value::Array(recommend_source)),
        ]);
        let opts = RequestOptions {
            url: "/recommend".into(),
            data: Some(Value::Object(data)),
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "songlistairec.kugou.com")]),
            clear_default_params: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /album
fn m_album(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let data: Vec<Value> = q_str(&query, "album_id")
            .split(',')
            .map(|s| json!({ "album_id": s, "album_name": "", "author_name": "" }))
            .collect();
        let dfid = dfid_of(&query);
        // userid/token 均 cookie 优先
        let userid = {
            let c = ck_str(&query, "userid");
            if !c.is_empty() { c } else { q_str(&query, "userid") }
        };
        let token = {
            let c = ck_str(&query, "token");
            if !c.is_empty() { c } else { q_str(&query, "token") }
        };
        let mut data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clienttime", Value::from(date_time)),
            ("clientver", Value::from(use_clientver())),
            ("data", Value::Array(data)),
            ("dfid", sv(dfid)),
            ("fields", sv(q_str(&query, "fields"))),
            ("key", sv(sign_params_key(&date_time.to_string(), None, None))),
            ("mid", mid_of(&query)),
        ]);
        if !token.is_empty() {
            data_map.insert("token".into(), sv(token));
        }
        if !userid.is_empty() {
            data_map.insert("userid".into(), sv(userid));
        }
        let opts = RequestOptions {
            base_url: Some("http://kmr.service.kugou.com".into()),
            url: "/v1/album".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[
                ("x-router", "kmr.service.kugou.com"),
                ("Content-Type", "application/json"),
            ]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /album/shop
fn m_album_shop(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/zhuanjidata/v3/album_shop_v2/get_classify_data".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /album/songs
fn m_album_songs(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("album_id", sv(q_str(&query, "id"))),
            ("is_buy", sv(q_str(&query, "is_buy"))),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/v1/album_audio/lite".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "openapi.kugou.com"), ("kg-tid", "255")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/albums
fn m_artist_albums(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let sort = if q_str(&query, "sort") == "hot" { 3 } else { 1 };
        let data_map = obj(vec![
            ("author_id", sv(q_str(&query, "id"))),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
            ("sort", Value::from(sort)),
            ("category", Value::from(1)),
            ("area_code", sv("all".into())),
        ]);
        let opts = RequestOptions {
            url: "/kmr/v1/author/albums".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "openapi.kugou.com"), ("kg-tid", "36")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/audios
fn m_artist_audios(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let clienttime = now_sec();
        let sort = if q_str(&query, "sort") == "hot" { 1 } else { 2 };
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("mid", mid_of(&query)),
            ("clienttime", Value::from(clienttime)),
            ("key", sv(sign_params_key(&clienttime.to_string(), None, None))),
            ("author_id", sv(q_str(&query, "id"))),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
            ("sort", Value::from(sort)),
            ("area_code", sv("all".into())),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://openapi.kugou.com".into()),
            url: "/kmr/v1/audio_group/author".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "openapi.kugou.com"), ("kg-tid", "220")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/detail
fn m_artist_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data = obj(vec![("author_id", sv(q_str(&query, "id")))]);
        let opts = RequestOptions {
            url: "/kmr/v3/author".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "openapi.kugou.com"), ("kg-tid", "36")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/follow
fn m_artist_follow(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let singerid = q_num(&query, "id");
        let token = token_of(&query);
        let userid = userid_of(&query);
        let clienttime = now_sec();
        let enc_data = serde_json::to_string(&json!({ "singerid": singerid, "token": token })).unwrap();
        let encrypt = crypto_aes_encrypt(&enc_data, None, None);
        let p_data = serde_json::to_string(&json!({ "clienttime": clienttime, "key": encrypt.key })).unwrap();
        let p = rsa_encrypt2(&p_data);
        let data = obj(vec![
            ("plat", Value::from(0)),
            ("userid", Value::from(userid)),
            ("singerid", Value::from(singerid)),
            ("source", Value::from(7)),
            ("p", sv(p)),
            ("params", sv(encrypt.str)),
        ]);
        let opts = RequestOptions {
            url: "/followservice/v3/follow_singer".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params: obj(vec![("clienttime", Value::from(clienttime))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/follow/newsongs
fn m_artist_follow_newsongs(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let last_album_id = s_or_num(&query, "last_album_id", 0);
        let page_size = s_or_num(&query, "pagesize", 30);
        // params.opt_sort === 2 恒为 false（字符串 vs 数字）—— 复刻，恒为 1
        let params_map = obj(vec![
            ("last_album_id", last_album_id.clone()),
            ("page_size", page_size),
            ("opt_sort", Value::from(1)),
        ]);
        let data = obj(vec![("last_album_id", last_album_id)]);
        let opts = RequestOptions {
            url: "/feed/v1/follow/newsong_album_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params: params_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/honour
fn m_artist_honour(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params_map = obj(vec![
            ("singer_id", sv(q_str(&query, "id"))),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
        ]);
        let opts = RequestOptions {
            base_url: Some("http://h5activity.kugou.com".into()),
            url: "/v1/query_singer_honour_detail".into(),
            method: "POST".into(),
            params: params_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/lists
fn m_artist_lists(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params_map = obj(vec![
            ("musician", n_of(&query, "musician")),
            ("sextype", s_or_num(&query, "sextypes", 0)),
            ("showtype", Value::from(2)),
            ("type", s_or_num(&query, "type", 0)),
            ("hotsize", n_or(&query, "hotsize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/ocean/v6/singer/list".into(),
            method: "GET".into(),
            params: params_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/unfollow
fn m_artist_unfollow(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let singerid = q_str(&query, "id");
        let token = token_of(&query);
        let userid_v: Value = {
            let p = q_str(&query, "userid");
            if !p.is_empty() {
                Value::String(p)
            } else {
                let c = ck_str(&query, "userid");
                if !c.is_empty() { Value::String(c) } else { Value::from(0) }
            }
        };
        let clienttime = now_sec();
        let enc_data = serde_json::to_string(&json!({ "singerid": singerid, "token": token })).unwrap();
        let encrypt = crypto_aes_encrypt(&enc_data, None, None);
        let p_data = serde_json::to_string(&json!({ "clienttime": clienttime, "key": encrypt.key })).unwrap();
        let p = rsa_encrypt2(&p_data);
        let data = obj(vec![
            ("plat", Value::from(0)),
            ("userid", userid_v),
            ("singerid", sv(singerid)),
            ("source", Value::from(7)),
            ("p", sv(p)),
            ("params", sv(encrypt.str)),
        ]);
        let opts = RequestOptions {
            url: "/followservice/v3/unfollow_singer".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /artist/videos
fn m_artist_videos(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let tag = q_str_or(&query, "tag", "all");
        let tag_idx: Value = match tag.as_str() {
            "official" => Value::from(18),
            "live" => Value::from(20),
            "fan" => Value::from(23),
            "artist" => Value::from(42419),
            _ => sv("".into()),
        };
        let params_map = obj(vec![
            ("author_id", sv(q_str(&query, "id"))),
            ("is_fanmade", sv("".into())),
            ("tag_idx", tag_idx),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://openapicdn.kugou.com".into()),
            url: "/kmr/v1/author/videos".into(),
            method: "GET".into(),
            params: params_map,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /audio
fn m_audio(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let data: Vec<Value> = q_str(&query, "hash")
            .split(',')
            .map(|s| json!({ "hash": s, "audio_id": 0 }))
            .collect();
        let dfid = dfid_of(&query);
        let userid = {
            let c = ck_str(&query, "userid");
            if !c.is_empty() { c } else { q_str(&query, "userid") }
        };
        let token = {
            let c = ck_str(&query, "token");
            if !c.is_empty() { c } else { q_str(&query, "token") }
        };
        let mut data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clienttime", Value::from(date_time)),
            ("clientver", Value::from(use_clientver())),
            ("data", Value::Array(data)),
            ("dfid", sv(dfid)),
            ("key", sv(sign_params_key(&date_time.to_string(), None, None))),
            ("mid", mid_of(&query)),
        ]);
        if !token.is_empty() {
            data_map.insert("token".into(), sv(token));
        }
        if !userid.is_empty() {
            data_map.insert("userid".into(), sv(userid));
        }
        let opts = RequestOptions {
            base_url: Some("http://kmr.service.kugou.com".into()),
            url: "/v1/audio/audio".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[
                ("x-router", "kmr.service.kugou.com"),
                ("Content-Type", "application/json"),
            ]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /audio/accompany/matching —— 自定义 sign + notSignature
fn m_audio_accompany_matching(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data_map = obj(vec![
            ("isteen", Value::from(0)),
            ("mixId", n_or(&query, "mixId", 0)),
            ("usemkv", Value::from(1)),
            ("platform", Value::from(2)),
            ("fileName", sv(q_str(&query, "fileName"))),
            ("hash", sv(q_str(&query, "hash"))),
            ("version", Value::from(12375)),
            ("appid", Value::from(use_appid())),
        ]);
        let ps = kv_sorted(&data_map, "&");
        let sign = crypto_md5(&format!("{}{}", ps, "*s&iN#G70*"))[8..24].to_string();
        data_map.insert("sign".into(), sv(sign));
        let opts = RequestOptions {
            base_url: Some("https://nsongacsing.kugou.com".into()),
            url: "/sing7/accompanywan/json/v2/cdn/optimal_matching_accompany_2_listen.do".into(),
            params: data_map,
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            clear_default_params: true,
            not_signature: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /audio/ktv/total —— 自定义 sign + notSignature
fn m_audio_ktv_total(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data_map = obj(vec![
            ("isteen", Value::from(0)),
            ("songId", n_of(&query, "songId")),
            ("usemkv", Value::from(1)),
            ("platform", Value::from(2)),
            ("singerName", sv(q_str(&query, "singerName"))),
            ("songHash", sv(q_str(&query, "songHash"))),
            ("version", Value::from(12375)),
            ("appid", Value::from(use_appid())),
        ]);
        let ps = kv_sorted(&data_map, "&");
        let sign = crypto_md5(&format!("{}{}", ps, "*s&iN#G70*"))[8..24].to_string();
        data_map.insert("sign".into(), sv(sign));
        let opts = RequestOptions {
            base_url: Some("https://acsing.service.kugou.com".into()),
            url: "/sing7/listenguide/json/v2/cdn/listenguide/get_total_opus_num_v02.do".into(),
            params: data_map,
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            clear_default_params: true,
            not_signature: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /audio/related —— 自定义 signature（android 密钥 + web 风格拼接）
fn m_audio_related(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let show_detail =
            q_get(&query, "show_detail").is_some() && q_num(&query, "show_detail") == 0;
        let mut data_map = obj(vec![
            ("album_audio_id", n_of(&query, "album_audio_id")),
            ("appid", Value::from(1005)),
            ("area_code", Value::from(1)),
            ("clientver", Value::from(12329)),
        ]);
        if !show_detail {
            data_map.insert("page".into(), s_or_num(&query, "page", 1));
            data_map.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
            data_map.insert("show_input".into(), Value::from(1));
            data_map.insert("show_type".into(), s_or_num(&query, "show_type", 0));
            let sort = match q_str(&query, "sort").as_str() {
                "all" => 1,
                "hot" => 2,
                "new" => 3,
                _ => 1,
            };
            data_map.insert("sort".into(), Value::from(sort));
            data_map.insert("type".into(), s_or_num(&query, "type", 0));
        }
        data_map.insert("version".into(), Value::from(1));
        let s = "OIlwieks28dk2k092lksi2UIkp";
        let ps = kv_sorted(&data_map, "");
        data_map.insert(
            "signature".into(),
            sv(crypto_md5(&format!("{}{}{}", s, ps, s))),
        );
        let opts = RequestOptions {
            base_url: Some("https://listkmrp3cdnretry.kugou.com".into()),
            url: if !show_detail {
                "/v3/album_audio/related".into()
            } else {
                "/v2/audio_related/total".into()
            },
            params: data_map,
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            clear_default_params: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /brush
fn m_brush(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid: i64 = {
            let c = ck_str(&query, "userid");
            if !c.is_empty() { c.parse().unwrap_or(0) } else { q_num(&query, "userid") }
        };
        let vip_type: i64 = {
            let c = ck_str(&query, "vip_type");
            if !c.is_empty() { c.parse().unwrap_or(0) } else { q_num(&query, "vipType") }
        };
        let date_time = now_ms();
        let personal_recommend = obj(vec![
            ("userid", Value::from(userid)),
            ("appid", Value::from(use_appid())),
            ("playlist_ver", Value::from(2)),
            ("clienttime", Value::from(date_time)),
            ("mid", mid_of(&query)),
            ("new_sync_point", Value::from(date_time)),
            ("module_id", Value::from(1)),
            ("action", sv("login".into())),
            ("vip_type", Value::from(vip_type)),
            ("vip_flags", Value::from(3)),
            ("recommend_source_locked", Value::from(0)),
            ("song_pool_id", n_of(&query, "song_pool_id")),
            ("callerid", Value::from(0)),
            ("m_type", Value::from(1)),
            ("kguid", Value::from(userid)),
            ("platform", sv("ios".into())),
            ("area_code", Value::from(1)),
            ("fakem", sv("ca981cfc583a4c37f28d2d49000013c16a0a".into())),
            ("clientver", Value::from(11850)),
            ("mode", sv(q_str_or(&query, "mode", "normal"))),
            ("active_swtich", sv("on".into())),
            ("key", sv(sign_params_key(&date_time.to_string(), None, None))),
        ]);
        let data_map = obj(vec![
            ("behaviors", Value::Array(vec![])),
            (
                "abtest",
                json!({ "abtest": { "shuashua": { "commentcard": 2 } } }),
            ),
            ("personal_recommend_params", Value::Object(personal_recommend)),
        ]);
        let params_map = obj(vec![
            ("sort_type", Value::from(1)),
            ("platform", sv("ios".into())),
            ("page", Value::from(1)),
            ("content_ver", Value::from(4)),
            ("clientver", Value::from(11850)),
        ]);
        let opts = RequestOptions {
            url: "/genesisapi/v1/newepoch_song_rec/feed".into(),
            data: Some(Value::Object(data_map)),
            params: params_map,
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /captcha/sent —— cookie 仅传 {mid: KUGOU_API_MID}
fn m_captcha_sent(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("businessid", Value::from(5)),
            ("mobile", sv(q_str(&query, "mobile"))),
            ("plat", Value::from(3)),
        ]);
        let cookie = obj(vec![("mid", mid_of(&query))]);
        let opts = RequestOptions {
            base_url: Some("http://login.user.kugou.com".into()),
            url: "/v7/send_mobile_code".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/album
fn m_comment_album(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params_map = obj(vec![
            ("childrenid", sv(q_str(&query, "id"))),
            ("need_show_image", Value::from(1)),
            ("p", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("show_classify", s_or_num(&query, "show_classify", 1)),
            ("show_hotword_list", s_or_num(&query, "show_hotword_list", 1)),
            ("code", sv("94f1792ced1df89aa68a7939eaf2efca".into())),
        ]);
        let opts = RequestOptions {
            url: "/m.comment.service/v1/cmtlist".into(),
            encrypt_type: EncryptType::Android,
            method: "POST".into(),
            params: params_map,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/count —— web 签名
fn m_comment_count(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params_map = obj(vec![
            ("r", sv("comments/getcommentsnum".into())),
            ("code", sv("fc4be23b4e972707f36b8a828a93ba8a".into())),
        ]);
        if !q_str(&query, "hash").is_empty() {
            params_map.insert("hash".into(), sv(q_str(&query, "hash")));
        } else if !q_str(&query, "special_id").is_empty() {
            params_map.insert("childrenid".into(), sv(q_str(&query, "special_id")));
        }
        let opts = RequestOptions {
            url: "/index.php".into(),
            encrypt_type: EncryptType::Web,
            method: "GET".into(),
            params: params_map,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "sum.comment.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/floor
fn m_comment_floor(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        const SONG_CODE: &str = "fc4be23b4e972707f36b8a828a93ba8a";
        const PLAYLIST_CODE: &str = "ca53b96fe5a1d9c22d71c8f522ef7c4f";
        const ALBUM_CODE: &str = "94f1792ced1df89aa68a7939eaf2efca";
        let is_non_empty = |v: Option<&Value>| -> bool {
            match v {
                None | Some(Value::Null) => false,
                Some(val) => {
                    let t = crate::jsval::js_to_string(val);
                    let t = t.trim();
                    !t.is_empty() && t != "null" && t != "undefined"
                }
            }
        };
        let resource_type = {
            let mut s = q_str(&query, "resource_type");
            if s.is_empty() {
                s = q_str(&query, "resourceType");
            }
            s.to_lowercase()
        };
        let code = if is_non_empty(q_get(&query, "code")) {
            q_str(&query, "code")
        } else if resource_type == "playlist" {
            PLAYLIST_CODE.to_string()
        } else if resource_type == "album" {
            ALBUM_CODE.to_string()
        } else {
            SONG_CODE.to_string()
        };
        let use_service_endpoint = resource_type == "playlist"
            || resource_type == "album"
            || code == PLAYLIST_CODE
            || code == ALBUM_CODE;
        let mut params_map = obj(vec![
            ("childrenid", sv(q_str(&query, "special_id"))),
            ("need_show_image", Value::from(1)),
            ("p", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("show_classify", s_or_num(&query, "show_classify", 1)),
            ("show_hotword_list", s_or_num(&query, "show_hotword_list", 1)),
            ("code", sv(code)),
            ("tid", sv(q_str(&query, "tid"))),
        ]);
        if is_non_empty(q_get(&query, "mixsongid")) {
            params_map.insert("mixsongid".into(), sv(q_str(&query, "mixsongid")));
        }
        let opts = RequestOptions {
            url: if use_service_endpoint {
                "/m.comment.service/v1/hot_replylist".into()
            } else {
                "/mcomment/v1/hot_replylist".into()
            },
            encrypt_type: EncryptType::Android,
            method: "POST".into(),
            params: params_map,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/music
fn m_comment_music(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params_map = obj(vec![
            ("mixsongid", sv(q_str(&query, "mixsongid"))),
            ("need_show_image", Value::from(1)),
            ("p", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("show_classify", s_or_num(&query, "show_classify", 1)),
            ("show_hotword_list", s_or_num(&query, "show_hotword_list", 1)),
            ("extdata", sv("0".into())),
            ("code", sv("fc4be23b4e972707f36b8a828a93ba8a".into())),
        ]);
        let opts = RequestOptions {
            url: "/mcomment/v1/cmtlist".into(),
            encrypt_type: EncryptType::Android,
            method: "POST".into(),
            params: params_map,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/music/classify
fn m_comment_music_classify(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let sort_method: Value = if q_num(&query, "sort") == 2 {
            sv(q_str(&query, "sort"))
        } else {
            Value::from(1)
        };
        let params_map = obj(vec![
            ("mixsongid", sv(q_str(&query, "mixsongid"))),
            ("need_show_image", Value::from(1)),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("type_id", sv(q_str(&query, "type_id"))),
            ("extdata", sv("0".into())),
            ("code", sv("fc4be23b4e972707f36b8a828a93ba8a".into())),
            ("sort_method", sort_method),
        ]);
        let opts = RequestOptions {
            url: "/mcomment/v1/cmt_classify_list".into(),
            encrypt_type: EncryptType::Android,
            method: "POST".into(),
            params: params_map,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /comment/music/hotword
fn m_comment_music_hotword(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params_map = obj(vec![
            ("mixsongid", sv(q_str(&query, "mixsongid"))),
            ("need_show_image", Value::from(1)),
            ("p", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("hot_word", sv(q_str(&query, "hot_word"))),
            ("extdata", sv("0".into())),
            ("code", sv("fc4be23b4e972707f36b8a828a93ba8a".into())),
        ]);
        let opts = RequestOptions {
            url: "/mcomment/v1/get_hot_word".into(),
            encrypt_type: EncryptType::Android,
            method: "POST".into(),
            params: params_map,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 2 辅助 =================

/// 对应 `{ x: params.x }` 透传（无默认值）：存在(非 null)则以字符串插入，缺失/null 跳过。
/// （真实请求必然携带必填参数；仅缺失的边界场景与 Node 的 undefined 处理略有差异，
///  该场景本身即为无效请求，服务端会因缺参而拒绝，签名值无意义。）
fn put_pass(m: &mut Map<String, Value>, q: &Value, src: &str, dst: &str) {
    if let Some(v) = q_get(q, src) {
        if !v.is_null() {
            m.insert(dst.to_string(), sv(crate::jsval::js_to_string(v)));
        }
    }
}

/// JS 真值判断（0/""/false/null -> false；"0" 为真）
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `params?.cookie?.userid || params?.userid`（**cookie 优先**，保留原始类型）
fn userid_ck_first_opt(q: &Value) -> Option<Value> {
    if let Some(v) = ck_get(q, "userid") {
        if truthy(v) {
            return Some(v.clone());
        }
    }
    if let Some(v) = q_get(q, "userid") {
        if truthy(v) {
            return Some(v.clone());
        }
    }
    None
}

/// `params?.cookie?.token || params?.token`（**cookie 优先**，保留原始类型）
fn token_ck_first_opt(q: &Value) -> Option<Value> {
    if let Some(v) = ck_get(q, "token") {
        if truthy(v) {
            return Some(v.clone());
        }
    }
    if let Some(v) = q_get(q, "token") {
        if truthy(v) {
            return Some(v.clone());
        }
    }
    None
}

/// URLSearchParams 风格百分号解码（'+' -> 空格）
fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(h) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// URLSearchParams.get：返回首个匹配 key 的解码值
fn qs_get(s: &str, key: &str) -> Option<String> {
    let s = s.strip_prefix('?').unwrap_or(s);
    for pair in s.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if url_decode(k) == key {
            return Some(url_decode(v));
        }
    }
    None
}

// ================= 批次 2 模块 =================

/// /comment/playlist —— 歌曲/歌单评论（POST + query params）
fn m_comment_playlist(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        put_pass(&mut params, &query, "id", "childrenid");
        params.insert("need_show_image".into(), Value::from(1));
        params.insert("p".into(), s_or_num(&query, "page", 1));
        params.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
        params.insert("show_classify".into(), s_or_num(&query, "show_classify", 1));
        params.insert(
            "show_hotword_list".into(),
            s_or_num(&query, "show_hotword_list", 1),
        );
        params.insert(
            "code".into(),
            sv("ca53b96fe5a1d9c22d71c8f522ef7c4f".into()),
        );
        params.insert("content_type".into(), Value::from(0));
        params.insert("tag".into(), Value::from(5));
        let opts = RequestOptions {
            url: "/m.comment.service/v1/cmtlist".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /everyday/friend —— 好友推荐（固定 data + acsing baseURL + 数字型 pid 头）
fn m_everyday_friend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data = json!({
            "list": [{
                "user_id": 853927886i64,
                "mixsong_ids": [290083753i64,251724346,571554587,250126644,208831644,40328518,250504076,581706850,318347675,585258401,288481998,407414475,28239430,280584633,291957521,64556644,243149863,488725103,32114153,39951172,29019580,40397606,327507651,32029382,32218359,340353127,276448762,177071956,100031397,249251602]
            }]
        });
        let mut params = Map::new();
        params.insert("channel".into(), Value::from(130));
        params.insert("isteen".into(), Value::from(0));
        params.insert("platform".into(), Value::from(2));
        params.insert("usemkv".into(), Value::from(1));
        let mut headers = Map::new();
        headers.insert("pid".into(), Value::from(126556797i64));
        let opts = RequestOptions {
            base_url: Some("https://acsing.service.kugou.com".into()),
            url: "/sing7/relation/json/v3/friend_rec_by_using_song_list".into(),
            method: "POST".into(),
            data: Some(data),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /everyday/history —— 每日推荐历史（mode/platform 默认 + 可选 history_name/date）
fn m_everyday_history(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("mode".into(), sv(q_str_or(&query, "mode", "list")));
        params.insert("platform".into(), sv(q_str_or(&query, "platform", "ios")));
        let hn = q_str(&query, "history_name");
        if !hn.is_empty() {
            params.insert("history_name".into(), sv(hn));
        }
        let dt = q_str(&query, "date");
        if !dt.is_empty() {
            params.insert("date".into(), sv(dt));
        }
        let opts = RequestOptions {
            url: "/everyday/api/v1/get_history".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "everydayrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /everyday/recommend —— 每日推荐
fn m_everyday_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("platform".into(), sv(q_str_or(&query, "platform", "ios")));
        let opts = RequestOptions {
            url: "/everyday_song_recommend".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "everydayrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /everyday/style/recommend —— 每日曲风推荐（data:{} + 查询 tagids）
fn m_everyday_style_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        // `params.tagids ?? ''` —— 缺失/null 时为空串
        params.insert("tagids".into(), sv(q_str(&query, "tagids")));
        let opts = RequestOptions {
            url: "/everydayrec.service/everyday_style_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(Map::new())),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /favorite/count —— 收藏计数（GET）
fn m_favorite_count(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        put_pass(&mut params, &query, "mixsongids", "mixsongids");
        let opts = RequestOptions {
            url: "/count/v1/audio/mget_collect".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /fm/class —— FM 分类电台歌曲
fn m_fm_class(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let userid = userid_ck_first_opt(&query).unwrap_or(Value::from(0));
        let data = obj(vec![
            ("kguid", userid.clone()),
            ("clienttime", Value::from(date_time)),
            ("mid", mid_of(&query)),
            ("platform", sv("android".into())),
            ("clientver", Value::from(use_clientver())),
            ("uid", userid.clone()),
            ("get_tracker", Value::from(1)),
            (
                "key",
                sv(sign_params_key(&date_time.to_string(), None, None)),
            ),
            ("appid", Value::from(use_appid())),
        ]);
        let opts = RequestOptions {
            url: "/v1/class_fm_song".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "fm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /fm/image —— FM 电台封面
fn m_fm_image(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let dfid = dfid_of(&query);
        let userid = userid_ck_first_opt(&query);
        let token = token_ck_first_opt(&query);
        let fm_data: Vec<Value> = q_str(&query, "fmid")
            .split(',')
            .map(|s| {
                json!({ "fields": "imgUrl100,imgUrl50", "fmid": s, "fmtype": 2 })
            })
            .collect();
        let mut data = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clienttime", Value::from(date_time)),
            ("clientver", Value::from(use_clientver())),
            ("data", Value::Array(fm_data)),
            ("dfid", sv(dfid)),
            (
                "key",
                sv(sign_params_key(&date_time.to_string(), None, None)),
            ),
            ("mid", mid_of(&query)),
        ]);
        if let Some(u) = userid {
            data.insert("userid".into(), u);
        }
        if let Some(t) = token {
            data.insert("token".into(), t);
        }
        let opts = RequestOptions {
            url: "/v1/fm_info".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "fm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /fm/recommend —— FM 推荐
fn m_fm_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let data = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("clienttime", Value::from(date_time)),
            ("mid", mid_of(&query)),
            (
                "key",
                sv(sign_params_key(&date_time.to_string(), None, None)),
            ),
            ("rcmdsongcount", Value::from(1)),
            ("level", Value::from(0)),
            ("area_code", Value::from(1)),
            ("get_tracker", Value::from(1)),
            ("uid", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/v1/rcmd_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "fm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /fm/songs —— FM 电台歌曲列表（多个逗号分隔覆盖字段）
fn m_fm_songs(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let userid = userid_ck_first_opt(&query);

        // `params.type || 2` / `params.offset || -1` / `params.size || 20`
        let fmtype_default = s_or_num(&query, "type", 2);
        let offset_default = s_or_num(&query, "offset", -1);
        let size_default = s_or_num(&query, "size", 20);

        let fmids: Vec<String> = q_str(&query, "fmid")
            .split(',')
            .map(|s| s.to_string())
            .collect();
        let mut fm_data: Vec<Map<String, Value>> = fmids
            .iter()
            .map(|s| {
                obj(vec![
                    ("fmid", sv(s.clone())),
                    ("fmtype", fmtype_default.clone()),
                    ("offset", offset_default.clone()),
                    ("size", size_default.clone()),
                    ("singername", sv(String::new())),
                ])
            })
            .collect();

        // 逗号分隔覆盖：`(params.fmX || '').split(',').forEach((s,l)=> d[l].field = s || d[l].field)`
        let apply_override = |data: &mut Vec<Map<String, Value>>, src: &str, field: &str| {
            for (l, s) in q_str(&query, src).split(',').enumerate() {
                if l < data.len() && !s.is_empty() {
                    data[l].insert(field.to_string(), sv(s.to_string()));
                }
            }
        };
        apply_override(&mut fm_data, "fmtype", "fmtype");
        apply_override(&mut fm_data, "fmoffset", "offset");
        apply_override(&mut fm_data, "fmsize", "size");

        let fm_data_val: Vec<Value> = fm_data.into_iter().map(Value::Object).collect();
        let mut data = obj(vec![
            ("appid", Value::from(use_appid())),
            ("area_code", Value::from(1)),
            ("clienttime", Value::from(date_time)),
            ("clientver", Value::from(use_clientver())),
            ("data", Value::Array(fm_data_val)),
            ("get_tracker", Value::from(1)),
            (
                "key",
                sv(sign_params_key(&date_time.to_string(), None, None)),
            ),
            ("mid", mid_of(&query)),
        ]);
        if let Some(u) = userid {
            data.insert("uid".into(), u);
        }
        let opts = RequestOptions {
            url: "/v1/app_song_list_offset".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "fm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /ip —— 根据 IP id 获取歌曲/专辑/视频/歌手
fn m_ip(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data = Map::new();
        data.insert("is_publish".into(), Value::from(1));
        put_pass(&mut data, &query, "id", "ip_id");
        data.insert("sort".into(), Value::from(3));
        data.insert("page".into(), s_or_num(&query, "page", 1));
        data.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
        data.insert("query".into(), Value::from(1));
        let valid = ["audios", "albums", "videos", "author_list"];
        let typ = q_str(&query, "type");
        let typ = if valid.contains(&typ.as_str()) {
            typ
        } else {
            "audios".to_string()
        };
        let opts = RequestOptions {
            url: format!("/openapi/v1/ip/{}", typ),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /ip/dateil —— IP 详情（注意源文件名拼写 dateil）
fn m_ip_dateil(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let arr: Vec<Value> = q_str(&query, "id")
            .split(',')
            .map(|s| json!({ "ip_id": s }))
            .collect();
        let mut data = Map::new();
        data.insert("data".into(), Value::Array(arr));
        data.insert("is_publish".into(), Value::from(1));
        let opts = RequestOptions {
            url: "/openapi/v1/ip".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /ip/playlist —— 根据 IP 获取相关歌单
fn m_ip_playlist(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        put_pass(&mut params, &query, "id", "ip");
        params.insert("page".into(), s_or_num(&query, "page", 1));
        params.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
        let opts = RequestOptions {
            url: "/ocean/v6/pubsongs/list_info_for_ip".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /ip/zone —— IP 专区（GET，随后解析 special_link 注入 ip_id）
fn m_ip_zone(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/v1/zone/index".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "yuekucategory.kugou.com")]),
            ..Default::default()
        };
        let mut resp = use_axios(opts).await?;
        // if (resp.body.status == 1) 且 data.list 为数组：解析 special_link 里的 ip_id
        let status_ok = resp
            .body
            .get("status")
            .map(|v| v.as_i64() == Some(1) || v.as_str() == Some("1"))
            .unwrap_or(false);
        if status_ok {
            if let Some(list) = resp
                .body
                .get_mut("data")
                .and_then(|d| d.get_mut("list"))
                .and_then(|l| l.as_array_mut())
            {
                for item in list.iter_mut() {
                    let special = item
                        .get("special_link")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(sl) = special {
                        if let Some(path) = qs_get(&sl, "path") {
                            let ip_id_str = qs_get(&path, "ip_id").unwrap_or_default();
                            let n = ip_id_str.parse::<i64>().unwrap_or(0);
                            if let Some(o) = item.as_object_mut() {
                                o.insert("ip_id".into(), Value::from(n));
                            }
                        }
                    }
                }
            }
        }
        Ok(resp)
    })
}

/// /ip/zone/home —— IP 专区今日推荐
fn m_ip_zone_home(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        put_pass(&mut params, &query, "id", "id");
        params.insert("share".into(), Value::from(0));
        let opts = RequestOptions {
            url: "/v1/zone/home".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "yuekucategory.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 3 模块 =================

/// /kmr/audio/mv —— 根据 album_audio_id 获取 MV
fn m_kmr_audio_mv(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let resource: Vec<Value> = q_str(&query, "album_audio_id")
            .split(',')
            .map(|s| json!({ "album_audio_id": s }))
            .collect();
        let data = obj(vec![
            ("data", Value::Array(resource)),
            ("fields", sv(q_str(&query, "fields"))),
        ]);
        let mut headers = Map::new();
        headers.insert("x-router".into(), sv("openapi.kugou.com".into()));
        headers.insert("KG-TID".into(), Value::from(38));
        let opts = RequestOptions {
            url: "/kmr/v1/audio/mv".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /krm/audio —— 根据 album_audio_id 获取歌手/专辑/歌曲信息
fn m_krm_audio(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let resource: Vec<Value> = q_str(&query, "album_audio_id")
            .split(',')
            .map(|s| json!({ "entity_id": s.parse::<i64>().unwrap_or(0) }))
            .collect();
        let data = obj(vec![
            ("data", Value::Array(resource)),
            ("fields", sv(q_str_or(&query, "fields", "base"))),
        ]);
        let mut headers = Map::new();
        headers.insert("x-router".into(), sv("openapi.kugou.com".into()));
        headers.insert("KG-TID".into(), Value::from(238));
        let opts = RequestOptions {
            url: "/kmr/v2/audio".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /lastest/songs/listen —— 获取继续播放信息
fn m_lastest_songs_listen(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let token = token_of(&query);
        let userid = userid_of(&query);
        let data = obj(vec![
            ("area_code", sv("1".into())),
            (
                "sources",
                json!(["pc", "mobile", "tv", "car"]),
            ),
            ("userid", Value::from(userid)),
            ("ret_info", Value::from(1)),
            ("token", sv(token)),
            ("pagesize", Value::from(q_num_or(&query, "pagesize", 30))),
        ]);
        let opts = RequestOptions {
            url: "/playque/devque/v1/get_latest_songs".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/album/audios —— 长音频专辑歌曲
fn m_longaudio_album_audios(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data = Map::new();
        put_pass(&mut data, &query, "album_id", "album_id");
        data.insert("area_code".into(), Value::from(1));
        data.insert("tagid".into(), Value::from(0));
        data.insert("page".into(), s_or_num(&query, "page", 1));
        data.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
        let opts = RequestOptions {
            url: "/longaudio/v2/album_audios".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: {
                let mut h = Map::new();
                h.insert("x-router".into(), sv("openapi.kugou.com".into()));
                h.insert("KG-TID".into(), sv("78".into()));
                h
            },
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/album/detail —— 长音频专辑详情
fn m_longaudio_album_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let arr: Vec<Value> = q_str(&query, "album_id")
            .split(',')
            .map(|s| json!({ "album_id": s }))
            .collect();
        let data = obj(vec![
            ("data", Value::Array(arr)),
            ("show_album_tag", Value::from(1)),
            ("fields", sv("album_name,album_id,category,authors,sizable_cover,intro,author_name,trans_param,album_tag,mix_intro,full_intro,is_publish".into())),
        ]);
        let opts = RequestOptions {
            url: "/openapi/v2/broadcast".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("KG-TID", "78")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/daily/recommend —— 长音频每日推荐
fn m_longaudio_daily_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("module_id".into(), Value::from(1));
        params.insert("size".into(), s_or_num(&query, "pagesize", 30));
        params.insert("page".into(), s_or_num(&query, "page", 1));
        let opts = RequestOptions {
            url: "/longaudio/v1/home_new/daily_recommend".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/rank/recommend —— 长音频榜单推荐
fn m_longaudio_rank_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("platform".into(), sv("ios".into()));
        let opts = RequestOptions {
            url: "/longaudio/v1/home_new/rank_card_recommend".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/vip/recommend —— 长音频 VIP 推荐
fn m_longaudio_vip_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("position".into(), sv("2".into()));
        params.insert("clientver".into(), Value::from(12329));
        let data = obj(vec![("album_playlist", Value::Array(vec![]))]);
        let opts = RequestOptions {
            url: "/longaudio/v1/home_new/vip_select_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /longaudio/week/recommend —— 长音频每周新专推荐
fn m_longaudio_week_recommend(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("clientver".into(), Value::from(12329));
        let data = obj(vec![("album_playlist", Value::Array(vec![]))]);
        let opts = RequestOptions {
            url: "/longaudio/v1/home_new/week_new_albums_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 4 模块（login 系列） =================

// —— 固定 token 串（login.js 硬编码）——
const LOGIN_T1: &str = "562a6f12a6e803453647d16a08f5f0c2ff7eee692cba2ab74cc4c8ab47fc467561a7c6b586ce7dc46a63613b246737c03a1dc8f8d162d8ce1d2c71893d19f1d4b797685a4c6d3d81341cbde65e488c4829a9b4d42ef2df470eb102979fa5adcdd9b4eecfea8b909ff7599abeb49867640f10c3c70fc444effca9d15db44a9a6c907731e2bb0f22cd9b3536380169995693e5f0e2424e3378097d3813186e3fe96bbe7023808a0981b4e2b6135a76faac";
const LOGIN_T2: &str = "31c4daf4cf480169ccea1cb7d4a209295865a9d2b788510301694db229b87807469ea0d41b4d4b9173c2151da7294aeebfc9738df154bbdf11a4e117bb5dff6a3af8ce5ce333e681c1f29a44038f27567d58992eb81283e080778ac77db1400fdf49b7cf7e26be2e5af4da7830cc3be4";
const T3_FIXED: &str = "MCwwLDAsMCwwLDAsMCwwLDA=";

// —— lite 版 t1/t2 派生密钥（login_cellphone.js / login_token.js 硬编码）——
const LITE_T1_KEY: &str = "5e4ef500e9597fe004bd09a46d8add98";
const LITE_T1_IV: &str = "04bd09a46d8add98";
const LITE_T2_KEY: &str = "fd14b35e3f81af3817a20ae7adae7020";
const LITE_T2_IV: &str = "17a20ae7adae7020";

// —— login_token.js 刷新密钥 ——
const TOKEN_KEY: &str = "90b8382a1bb4ccdcf063102053fd75b8";
const TOKEN_IV: &str = "f063102053fd75b8";
const TOKEN_LITE_KEY: &str = "c24f74ca2820225badc01946dba4fdf7";
const TOKEN_LITE_IV: &str = "adc01946dba4fdf7";

/// JSON.stringify 等价（对象/数组 -> 无空格 JSON）
fn json_str(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// cookie 模板取值：`${params.cookie?.X}` —— 缺失得到 "undefined"
fn ck_tmpl(q: &Value, key: &str) -> String {
    match ck_get(q, key) {
        Some(v) => crate::jsval::js_to_string(v),
        None => "undefined".to_string(),
    }
}

/// 读取 res.body.data[key] 的模板串：缺失 -> "undefined"（对应 `${data['x']}`）
fn data_tmpl(res: &Response, key: &str) -> String {
    match res.body.pointer(&format!("/data/{}", key)) {
        Some(v) => crate::jsval::js_to_string(v),
        None => "undefined".to_string(),
    }
}

/// 读取 res.body.data[key]，falsy 时回退 default（对应 `${data?.x || default}`）
fn data_or(res: &Response, key: &str, default: &str) -> String {
    match res.body.pointer(&format!("/data/{}", key)) {
        Some(v) if truthy(v) => crate::jsval::js_to_string(v),
        _ => default.to_string(),
    }
}

/// 复刻 login*.js 的 secu_params 解密 + 合并 body.data + 回写 cookie。
/// `push_token_on_string`: 字符串分支是否额外 push `token=...`（仅 login.js 为 true）。
fn apply_secu(res: &mut Response, key: &str, push_token_on_string: bool) {
    let secu = match res.body.pointer("/data/secu_params").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };
    let get_token = crypto_aes_decrypt(&secu, key, None);
    let mut cookies: Vec<String> = Vec::new();
    {
        let data = match res.body.get_mut("data").and_then(|d| d.as_object_mut()) {
            Some(d) => d,
            None => return,
        };
        match &get_token {
            Value::Object(tok) => {
                // res.body.data = { ...body.data, ...getToken }
                for (k, v) in tok {
                    data.insert(k.clone(), v.clone());
                    cookies.push(format!("{}={}", k, crate::jsval::js_to_string(v)));
                }
            }
            other => {
                data.insert("token".to_string(), other.clone());
                if push_token_on_string {
                    cookies.push(format!("token={}", crate::jsval::js_to_string(other)));
                }
            }
        }
    }
    res.cookie.extend(cookies);
}

/// /login —— 账号密码登录
fn m_login(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_now = now_ms();
        // encrypt = cryptoAesEncrypt({pwd, code:'', clienttime_ms})  (无 opt -> {str,key})
        let enc_obj = json!({
            "pwd": q_str(&query, "password"),
            "code": "",
            "clienttime_ms": date_now,
        });
        let encrypt = crypto_aes_encrypt(&json_str(&enc_obj), None, None);
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime_ms": date_now, "key": encrypt.key })),
            None,
        )
        .to_uppercase();

        let mut data = Map::new();
        data.insert("plat".into(), Value::from(1));
        data.insert("support_multi".into(), Value::from(1));
        data.insert("clienttime_ms".into(), Value::from(date_now));
        data.insert("t1".into(), sv(LOGIN_T1.into()));
        data.insert("t2".into(), sv(LOGIN_T2.into()));
        data.insert("t3".into(), sv(T3_FIXED.into()));
        // username: params?.username（undefined -> JSON.stringify 丢弃）
        if let Some(v) = q_get(&query, "username") {
            if !v.is_null() {
                data.insert("username".into(), sv(crate::jsval::js_to_string(v)));
            }
        }
        data.insert("params".into(), sv(encrypt.str.clone()));
        data.insert("pk".into(), sv(pk));

        let opts = RequestOptions {
            url: "/v9/login_by_pwd".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "login.user.kugou.com")]),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        // 仅当 status===1 且存在 secu_params 时回写 cookie 并 resolve
        if res.body.get("status").and_then(|v| v.as_i64()) == Some(1) {
            let has_secu = res
                .body
                .pointer("/data/secu_params")
                .map(truthy)
                .unwrap_or(false);
            if has_secu {
                apply_secu(&mut res, &encrypt.key, true);
                res.cookie.push(format!("userid={}", data_or(&res, "userid", "0")));
                res.cookie.push(format!("vip_type={}", data_or(&res, "vip_type", "0")));
                res.cookie.push(format!("vip_token={}", data_or(&res, "vip_token", "")));
            }
        }
        Ok(res)
    })
}

/// /login/cellphone —— 手机验证码登录
fn m_login_cellphone(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_ms();
        let is_lite = config::is_lite();

        // encrypt = cryptoAesEncrypt({mobile: params?.mobile||'', code: params?.code||''})
        let encrypt = crypto_aes_encrypt(
            &json_str(&json!({
                "mobile": q_str(&query, "mobile"),
                "code": q_str(&query, "code"),
            })),
            None,
            None,
        );

        // mobile = params?.mobile && `${sub(0,2)}*****${sub(10,11)}`
        let mobile_field: Option<Value> = match q_get(&query, "mobile") {
            None => None, // undefined -> 整个 mobile 值为 undefined -> 丢弃
            Some(v) => {
                let s = crate::jsval::js_to_string(v);
                if s.is_empty() {
                    Some(Value::String(String::new())) // '' && X = ''
                } else {
                    let first2: String = s.chars().take(2).collect();
                    let c10 = s.chars().nth(10).map(|c| c.to_string()).unwrap_or_default();
                    Some(Value::String(format!("{}*****{}", first2, c10)))
                }
            }
        };

        // dfid = params?.cookie?.dfid ?? randomString(24)（?? 仅 null/undefined 回退）
        let dfid = match ck_get(&query, "dfid") {
            Some(v) if !v.is_null() => crate::jsval::js_to_string(v),
            _ => random_string(24),
        };

        let t2 = crypto_aes_encrypt(
            &format!(
                "{}|0f607264fc6318a92b9e13c65db7cd3c|{}|{}|{}",
                ck_tmpl(&query, "KUGOU_API_GUID"),
                ck_tmpl(&query, "KUGOU_API_MAC"),
                ck_tmpl(&query, "KUGOU_API_DEV"),
                date_time
            ),
            Some(LITE_T2_KEY),
            Some(LITE_T2_IV),
        )
        .str;
        let t1 = crypto_aes_encrypt(&format!("|{}", date_time), Some(LITE_T1_KEY), Some(LITE_T1_IV)).str;
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime_ms": date_time, "key": encrypt.key })),
            None,
        )
        .to_uppercase();

        let mut data = Map::new();
        data.insert("plat".into(), Value::from(1));
        data.insert("support_multi".into(), Value::from(1));
        data.insert("t1".into(), if is_lite { sv(t1) } else { Value::from(0) });
        data.insert("t2".into(), if is_lite { sv(t2) } else { Value::from(0) });
        data.insert("clienttime_ms".into(), Value::from(date_time));
        if let Some(mv) = mobile_field {
            data.insert("mobile".into(), mv);
        }
        data.insert(
            "key".into(),
            sv(sign_params_key(&date_time.to_string(), None, None)),
        );
        data.insert("pk".into(), sv(pk));
        data.insert("params".into(), sv(encrypt.str.clone()));
        // if (params?.userid) dataMap.userid = params.userid
        if let Some(v) = q_get(&query, "userid") {
            if truthy(v) {
                data.insert("userid".into(), sv(crate::jsval::js_to_string(v)));
            }
        }
        if is_lite {
            data.insert("dfid".into(), sv(dfid));
            // dev = params.cookie?.KUGOU_API_DEV（undefined -> 丢弃）
            if let Some(v) = ck_get(&query, "KUGOU_API_DEV") {
                if !v.is_null() {
                    data.insert("dev".into(), sv(crate::jsval::js_to_string(v)));
                }
            }
            data.insert("gitversion".into(), sv("5f0b7c4".into()));
        } else {
            data.insert("t3".into(), sv(T3_FIXED.into()));
        }

        let opts = RequestOptions {
            base_url: Some("https://loginserviceretry.kugou.com".into()),
            url: "/v7/login_by_verifycode".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            headers: str_map(&[
                ("support-calm", "1"),
                ("User-Agent", "Android16-1070-11440-130-0-LOGIN-wifi"),
            ]),
            cookie: cookie_of(&query),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        if res.body.get("status").and_then(|v| v.as_i64()) == Some(1) {
            // secu_params 可选；字符串分支不 push token
            apply_secu(&mut res, &encrypt.key, false);
            res.cookie.push(format!("t1={}", data_tmpl(&res, "t1")));
            res.cookie.push(format!("token={}", data_tmpl(&res, "token")));
            res.cookie.push(format!("userid={}", data_or(&res, "userid", "0")));
            res.cookie.push(format!("vip_type={}", data_or(&res, "vip_type", "0")));
            res.cookie.push(format!("vip_token={}", data_or(&res, "vip_token", "")));
        }
        Ok(res)
    })
}

/// /login/token —— 刷新登录
fn m_login_token(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_now = now_ms();
        let is_lite = config::is_lite();
        let token = token_of(&query); // params.token || cookie.token || ''
        // userid: params?.userid || cookie?.userid || '0'（默认字符串 '0'）
        let userid_v: Value = if let Some(v) = q_get(&query, "userid").filter(|v| truthy(v)) {
            sv(crate::jsval::js_to_string(v))
        } else if let Some(v) = ck_get(&query, "userid").filter(|v| truthy(v)) {
            sv(crate::jsval::js_to_string(v))
        } else {
            sv("0".into())
        };

        // encrypt = cryptoAesEncrypt({clienttime, token}, {key,iv}) -> 显式 key -> 裸 hex
        let (ek, ei) = if is_lite {
            (TOKEN_LITE_KEY, TOKEN_LITE_IV)
        } else {
            (TOKEN_KEY, TOKEN_IV)
        };
        let encrypt = crypto_aes_encrypt(
            &json_str(&json!({ "clienttime": date_now / 1000, "token": token })),
            Some(ek),
            Some(ei),
        );
        // encryptParams = cryptoAesEncrypt({}) -> {str,key}
        let encrypt_params = crypto_aes_encrypt("{}", None, None);
        // pk = cryptoRSAEncrypt({clienttime_ms, key: encryptParams.key})  —— 注意无 toUpperCase
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime_ms": date_now, "key": encrypt_params.key })),
            None,
        );

        let t2 = crypto_aes_encrypt(
            &format!(
                "{}|0f607264fc6318a92b9e13c65db7cd3c|{}|{}|{}",
                ck_tmpl(&query, "KUGOU_API_GUID"),
                ck_tmpl(&query, "KUGOU_API_MAC"),
                ck_tmpl(&query, "KUGOU_API_DEV"),
                date_now
            ),
            Some(LITE_T2_KEY),
            Some(LITE_T2_IV),
        )
        .str;
        let t1_plain = match ck_get(&query, "t1") {
            Some(v) if truthy(v) => format!("{}|{}", crate::jsval::js_to_string(v), date_now),
            _ => format!("|{}", date_now),
        };
        let t1 = crypto_aes_encrypt(&t1_plain, Some(LITE_T1_KEY), Some(LITE_T1_IV)).str;

        let dfid = {
            let c = ck_str(&query, "dfid");
            if c.is_empty() {
                "-".to_string()
            } else {
                c
            }
        };

        let mut data = Map::new();
        data.insert("dfid".into(), sv(dfid));
        data.insert("p3".into(), sv(encrypt.str.clone()));
        data.insert("plat".into(), Value::from(1));
        data.insert("t1".into(), if is_lite { sv(t1) } else { Value::from(0) });
        data.insert("t2".into(), if is_lite { sv(t2) } else { Value::from(0) });
        data.insert("t3".into(), sv(T3_FIXED.into()));
        data.insert("pk".into(), sv(pk));
        data.insert("params".into(), sv(encrypt_params.str.clone()));
        data.insert("userid".into(), userid_v);
        data.insert("clienttime_ms".into(), Value::from(date_now));
        if is_lite {
            if let Some(v) = ck_get(&query, "KUGOU_API_DEV") {
                if !v.is_null() {
                    data.insert("dev".into(), sv(crate::jsval::js_to_string(v)));
                }
            }
        }

        let opts = RequestOptions {
            base_url: Some("http://login.user.kugou.com".into()),
            url: "/v5/login_by_token".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            cookie: cookie_of(&query),
            encrypt_type: EncryptType::Android,
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        if res.body.get("status").and_then(|v| v.as_i64()) == Some(1) {
            apply_secu(&mut res, &encrypt_params.key, false);
            res.cookie.push(format!("t1={}", data_tmpl(&res, "t1")));
            res.cookie.push(format!("token={}", data_tmpl(&res, "token")));
            res.cookie.push(format!("userid={}", data_or(&res, "userid", "0")));
            res.cookie.push(format!("vip_type={}", data_or(&res, "vip_type", "0")));
            res.cookie.push(format!("vip_token={}", data_or(&res, "vip_token", "")));
        }
        Ok(res)
    })
}

/// /login/qr/check —— 二维码状态检测（status==4 时回写 cookie）
fn m_login_qr_check(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("plat".into(), Value::from(4));
        params.insert("appid".into(), Value::from(config::use_appid()));
        params.insert("srcappid".into(), Value::from(config::CONFIG.srcappid));
        params.insert(
            "qrcode".into(),
            query.get("key").cloned().unwrap_or(Value::Null),
        );

        let opts = RequestOptions {
            base_url: Some("https://login-user.kugou.com".into()),
            url: "/v2/get_userinfo_qrcode".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Web,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        // resp.body?.data?.status == 4（宽松等于：数字 4 或字符串 "4"）
        let is_4 = match res.body.pointer("/data/status") {
            Some(Value::Number(n)) => n.as_i64() == Some(4),
            Some(Value::String(s)) => s == "4",
            _ => false,
        };
        if is_4 {
            res.cookie.push(format!("token={}", data_tmpl(&res, "token")));
            res.cookie.push(format!("userid={}", data_tmpl(&res, "userid")));
        }
        Ok(res)
    })
}

/// /login/device —— 获取登录设备列表
fn m_login_device(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let clienttime_ms = now_ms();
        // encrypt = cryptoAesEncrypt({token: params.token || params.cookie?.token})
        //   token falsy -> JSON.stringify 丢弃该 key -> '{}'
        let mut eo = Map::new();
        let tokv = q_get(&query, "token")
            .filter(|v| truthy(v))
            .cloned()
            .or_else(|| ck_get(&query, "token").filter(|v| truthy(v)).cloned());
        if let Some(v) = &tokv {
            eo.insert("token".into(), sv(crate::jsval::js_to_string(v)));
        }
        let encrypt = crypto_aes_encrypt(&json_str(&Value::Object(eo)), None, None);
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime_ms": clienttime_ms, "key": encrypt.key })),
            None,
        )
        .to_uppercase();

        // userid: params.userid || params.cookie?.userid || 0（默认数字 0）
        let userid_v: Value = if let Some(v) = q_get(&query, "userid").filter(|v| truthy(v)) {
            sv(crate::jsval::js_to_string(v))
        } else if let Some(v) = ck_get(&query, "userid").filter(|v| truthy(v)) {
            sv(crate::jsval::js_to_string(v))
        } else {
            Value::from(0)
        };

        let mut data = Map::new();
        data.insert("plat".into(), Value::from(1));
        data.insert("userid".into(), userid_v);
        data.insert("clienttime_ms".into(), Value::from(clienttime_ms));
        data.insert("pk".into(), sv(pk));
        data.insert("params".into(), sv(encrypt.str.clone()));

        let opts = RequestOptions {
            base_url: Some("https://userinfoservice.kugou.com".into()),
            url: "/v2/get_dev".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /login/device/kick —— 登出指定设备
/// **复刻 Node 源码 bug**：login_device_kick.js 引用未导入的 calculateMid 及
/// 未定义的 uuid/dfid/userid/guid，运行时抛 ReferenceError，被 server 捕获后返回 404。
/// 这里直接返回 body 为 null 的 Err，令 server 走 404 信封分支。
fn m_login_device_kick(_query: Value, _use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        Err(Response {
            status: 500,
            body: Value::Null,
            cookie: Vec::new(),
            headers: std::collections::HashMap::new(),
        })
    })
}

// ================= 批次 5 模块（images / search / recommend） =================

/// encodeURIComponent 等价：仅保留 A-Za-z0-9 - _ . ! ~ * ' ( )，其余 UTF-8 字节转 %XX（大写）
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')')
        {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

/// 对应 images/images_audio：`Object.keys().sort().map(k=>k+'='+encodeURIComponent(obj?JSON.stringify:v))`
fn inline_sorted_query(map: &Map<String, Value>) -> String {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys.iter()
        .map(|k| {
            let v = &map[*k];
            let vs = match v {
                Value::Object(_) | Value::Array(_) => json_str(v),
                _ => crate::jsval::js_to_string(v),
            };
            format!("{}={}", k, encode_uri_component(&vs))
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// `params?.x || params?.cookie?.x || fallback`（params/cookie 值转字符串；fallback 原样）
fn q_or_ck_or(q: &Value, key: &str, fallback: Value) -> Value {
    if let Some(v) = q_get(q, key).filter(|v| truthy(v)) {
        return sv(crate::jsval::js_to_string(v));
    }
    if let Some(v) = ck_get(q, key).filter(|v| truthy(v)) {
        return sv(crate::jsval::js_to_string(v));
    }
    fallback
}

/// /images —— 专辑/歌手图片（notSign + URL 内联 query）
fn m_images(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let hash = q_str(&query, "hash");
        let mut data: Vec<Value> = hash
            .split(',')
            .map(|s| json!({ "album_id": 0, "hash": s, "album_audio_id": 0 }))
            .collect();
        // album_id 覆盖：s || 0
        for (i, s) in q_str(&query, "album_id").split(',').enumerate() {
            if i < data.len() {
                data[i]["album_id"] = if s.is_empty() { Value::from(0) } else { sv(s.into()) };
            }
        }
        for (i, s) in q_str(&query, "album_audio_id").split(',').enumerate() {
            if i < data.len() {
                data[i]["album_audio_id"] =
                    if s.is_empty() { Value::from(0) } else { sv(s.into()) };
            }
        }

        let mut params_map = Map::new();
        params_map.insert("album_image_type".into(), sv("-3".into()));
        params_map.insert("appid".into(), Value::from(config::use_appid()));
        params_map.insert("clientver".into(), Value::from(config::use_clientver()));
        params_map.insert("author_image_type".into(), sv("3,4,5".into()));
        params_map.insert("count".into(), s_or_num(&query, "count", 5));
        params_map.insert("data".into(), Value::Array(data));
        params_map.insert("isCdn".into(), Value::from(1));
        params_map.insert("publish_time".into(), Value::from(1));

        let qs = inline_sorted_query(&params_map);
        let signature = sign::signature_android(&params_map, "");

        let opts = RequestOptions {
            base_url: Some("https://expendablekmr.kugou.com".into()),
            url: format!("/container/v2/image?{}", qs),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            params: obj(vec![("signature", sv(signature))]),
            cookie: cookie_of(&query),
            not_signature: true,
            clear_default_params: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /images/audio —— 音频图片（notSign + URL 内联 query）
fn m_images_audio(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let hash = q_str(&query, "hash");
        let mut data: Vec<Value> = hash
            .split(',')
            .map(|s| json!({ "audio_id": 0, "hash": s, "album_audio_id": 0, "filename": "" }))
            .collect();
        for (i, s) in q_str(&query, "audio_id").split(',').enumerate() {
            if i < data.len() {
                data[i]["audio_id"] = if s.is_empty() { Value::from(0) } else { sv(s.into()) };
            }
        }
        for (i, s) in q_str(&query, "album_audio_id").split(',').enumerate() {
            if i < data.len() {
                data[i]["album_audio_id"] =
                    if s.is_empty() { Value::from(0) } else { sv(s.into()) };
            }
        }
        // filename 覆盖：s（不做 || 0）
        for (i, s) in q_str(&query, "filename").split(',').enumerate() {
            if i < data.len() {
                data[i]["filename"] = sv(s.into());
            }
        }

        let mut params_map = Map::new();
        params_map.insert("appid".into(), Value::from(config::use_appid()));
        params_map.insert("clientver".into(), Value::from(config::use_clientver()));
        params_map.insert("count".into(), s_or_num(&query, "count", 5));
        params_map.insert("data".into(), Value::Array(data));
        params_map.insert("isCdn".into(), Value::from(1));
        params_map.insert("publish_time".into(), Value::from(1));
        params_map.insert("show_authors".into(), Value::from(1));

        let qs = inline_sorted_query(&params_map);
        let signature = sign::signature_android(&params_map, "");

        let opts = RequestOptions {
            base_url: Some("https://expendablekmr.kugou.com".into()),
            url: format!("/v2/author_image/audio?{}", qs),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            params: obj(vec![("signature", sv(signature))]),
            cookie: cookie_of(&query),
            not_signature: true,
            clear_default_params: true,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/complex —— 综合搜索
fn m_search_complex(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert("platform".into(), sv("AndroidFilter".into()));
        params.insert(
            "keyword".into(),
            query.get("keywords").cloned().unwrap_or(Value::Null),
        );
        params.insert("page".into(), s_or_num(&query, "page", 1));
        params.insert("pagesize".into(), s_or_num(&query, "pagesize", 30));
        params.insert("cursor".into(), Value::from(0));

        let opts = RequestOptions {
            base_url: Some("https://complexsearch.kugou.com".into()),
            url: "/v6/search/complex".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/default —— 默认搜索词
fn m_search_default(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = userid_of(&query);
        let vip_type = q_or_ck_or(&query, "vip_type", Value::from(65530));

        let mut data = Map::new();
        data.insert("plat".into(), Value::from(0));
        data.insert("userid".into(), Value::from(userid));
        data.insert("tags".into(), sv("{}".into()));
        data.insert("vip_type".into(), vip_type);
        data.insert("m_type".into(), Value::from(0));
        data.insert("own_ads".into(), Value::Object(Map::new()));
        data.insert("ability".into(), sv("3".into()));
        data.insert("sources".into(), Value::Array(vec![]));
        data.insert("bitmap".into(), Value::from(2));
        data.insert("mode".into(), sv("normal".into()));

        let opts = RequestOptions {
            url: "/searchnofocus/v1/search_no_focus_word".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            params: obj(vec![("clientver", Value::from(12329))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/hot —— 热搜
fn m_search_hot(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/api/v3/search/hot_tab".into(),
            method: "GET".into(),
            params: obj(vec![("navid", Value::from(1)), ("plat", Value::from(2))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "msearch.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/mixed —— 综合搜索（IOSFilter）
fn m_search_mixed(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let time = now_ms();
        let requestid = format!(
            "{}_0",
            crypto_md5(&format!("bdaa53d04e7475feb9024164a47032f9{}", time))
        );
        let mut params = Map::new();
        params.insert("ab_tag".into(), Value::from(0));
        params.insert("ability".into(), Value::from(511));
        params.insert("albumhide".into(), Value::from(0));
        params.insert("apiver".into(), Value::from(22));
        params.insert("area_code".into(), Value::from(1));
        params.insert("clientver".into(), Value::from(20125));
        params.insert("cursor".into(), Value::from(0));
        params.insert("is_gpay".into(), Value::from(0));
        params.insert("iscorrection".into(), Value::from(1));
        params.insert(
            "keyword".into(),
            query.get("keyword").cloned().unwrap_or(Value::Null),
        );
        params.insert("nocollect".into(), Value::from(0));
        params.insert("osversion".into(), Value::from(16.5));
        params.insert("platform".into(), sv("IOSFilter".into()));
        params.insert("recver".into(), Value::from(2));
        params.insert("req_ai".into(), Value::from(1));
        params.insert("requestid".into(), sv(requestid));
        params.insert("search_ability".into(), Value::from(3));
        params.insert("sec_aggre".into(), Value::from(1));
        params.insert("sec_aggre_bitmap".into(), Value::from(0));
        params.insert("style_type".into(), Value::from(3));
        params.insert("tag".into(), sv("em".into()));

        let opts = RequestOptions {
            url: "/v3/search/mixed".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            headers: str_map(&[
                ("x-router", "complexsearch.kugou.com"),
                ("kg-clienttimems", &time.to_string()),
            ]),
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /search/suggest —— 搜索建议
fn m_search_suggest(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut params = Map::new();
        params.insert(
            "keyword".into(),
            query.get("keywords").cloned().unwrap_or(Value::Null),
        );
        params.insert("AlbumTipCount".into(), s_or_num(&query, "albumTipCount", 10));
        params.insert(
            "CorrectTipCount".into(),
            s_or_num(&query, "correctTipCount", 10),
        );
        params.insert("MVTipCount".into(), s_or_num(&query, "mvTipCount", 10));
        params.insert("MusicTipCount".into(), s_or_num(&query, "musicTipCount", 10));
        params.insert("radiotip".into(), Value::from(1));

        let opts = RequestOptions {
            url: "/v2/getSearchTip".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "searchtip.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /recommend/songs —— 每日推荐歌曲
fn m_recommend_songs(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut data = Map::new();
        data.insert("platform".into(), sv(q_str_or(&query, "platform", "android")));
        data.insert("userid".into(), q_or_ck_or(&query, "userid", sv("0".into())));

        let opts = RequestOptions {
            url: "/everyday_song_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "everydayrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 6 模块（rank / singer / song） =================

/// /rank/audio —— 排行榜音乐列表
fn m_rank_audio(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("show_portrait_mv", Value::from(1)),
            ("show_type_total", Value::from(1)),
            ("filter_original_remarks", Value::from(1)),
            ("area_code", Value::from(1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("rank_cid", s_or_num(&query, "rank_cid", 0)),
            ("type", Value::from(1)),
            ("page", s_or_num(&query, "page", 1)),
            ("rank_id", sv(q_str(&query, "rankid"))),
        ]);
        let opts = RequestOptions {
            url: "/openapi/kmr/v2/rank/audio".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("kg-tid", "369")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /rank/info —— 排行榜详情
fn m_rank_info(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("rank_cid", s_or_num(&query, "rank_cid", 0)),
            ("rankid", sv(q_str(&query, "rankid"))),
            ("with_album_img", s_or_num(&query, "album_img", 1)),
            ("zone", sv(q_str(&query, "zone"))),
        ]);
        let opts = RequestOptions {
            url: "/ocean/v6/rank/info".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /rank/list —— 排行榜列表
fn m_rank_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("plat", Value::from(2)),
            ("withsong", s_or_num(&query, "withsong", 1)),
            ("parentid", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/ocean/v6/rank/list".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /rank/top —— 排行榜推荐列表
fn m_rank_top(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/mobileservice/api/v5/rank/rec_rank_list".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /rank/vol —— 排行榜往期列表
fn m_rank_vol(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("rank_cid", s_or_num(&query, "rank_cid", 0)),
            ("rankid", sv(q_str(&query, "rankid"))),
            ("ranktype", Value::from(1)),
            ("type", Value::from(0)),
            ("plat", Value::from(2)),
        ]);
        let opts = RequestOptions {
            url: "/ocean/v6/rank/vol".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /singer/list —— 歌手列表
fn m_singer_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("hotsize", s_or_num(&query, "hotsize", 200)),
            ("musician", Value::from(0)),
            ("sextype", s_or_num(&query, "sextype", 0)),
            ("showtype", Value::from(2)),
            ("type", s_or_num(&query, "type", 0)),
        ]);
        let opts = RequestOptions {
            url: "/ocean/v6/singer/list".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /song/climax —— 音频高潮部分
fn m_song_climax(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // (params?.hash || '').split(',').map(s => ({ hash: s }))
        let hash = q_str(&query, "hash");
        let data: Vec<Value> = hash.split(',').map(|s| json!({ "hash": s })).collect();
        let params = obj(vec![("data", sv(json_str(&Value::Array(data))))]);
        let opts = RequestOptions {
            base_url: Some("https://expendablekmrcdn.kugou.com".into()),
            url: "/v1/audio_climax/audio".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /song/ranking —— 歌曲成绩单
fn m_song_ranking(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![("album_audio_id", sv(q_str(&query, "album_audio_id")))]);
        let opts = RequestOptions {
            url: "/grow/v1/song_ranking/play_page/ranking_info".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /song/ranking/filter —— 歌曲成绩单（解锁筛选）
fn m_song_ranking_filter(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("album_audio_id", sv(q_str(&query, "album_audio_id"))),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/grow/v1/song_ranking/unlock/v2/ranking_filter".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /song/url/new —— 获取歌曲播放地址（tracker v6/priv_url）
fn m_song_url_new(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let clienttime_ms = now_ms();
        let vip_token = {
            let a = q_str(&query, "vip_token");
            if !a.is_empty() { a } else { ck_str(&query, "vip_token") }
        };
        let token = {
            let a = q_str(&query, "token");
            if !a.is_empty() { a } else { ck_str(&query, "token") }
        };
        let userid = userid_of(&query); // params?.userid || cookie?.userid || '0'
        // dfid: params?.dfid || cookie?.dfid || randomString(24)
        let dfid = {
            let a = q_str(&query, "dfid");
            if !a.is_empty() {
                a
            } else {
                let c = ck_str(&query, "dfid");
                if !c.is_empty() { c } else { random_string(24) }
            }
        };
        // vip_type: cookie?.vip_type || params?.vipType || 0
        let vip_type: Value = {
            let c = ck_str(&query, "vip_type");
            if !c.is_empty() {
                sv(c)
            } else {
                let p = q_str(&query, "vipType");
                if !p.is_empty() { sv(p) } else { Value::from(0) }
            }
        };

        let key_src = format!(
            "{}185672dd44712f60bb1736df5a377e82{}{}{}",
            q_str(&query, "hash"),
            use_appid(),
            ck_tmpl(&query, "KUGOU_API_MID"),
            userid
        );
        let key = crypto_md5(&key_src);

        let resource = obj(vec![
            ("album_audio_id", sv(q_str(&query, "album_audio_id"))),
            ("collect_list_id", sv("3".into())),
            ("collect_time", Value::from(clienttime_ms)),
            ("hash", sv(q_str(&query, "hash"))),
            ("id", Value::from(0)),
            ("page_id", Value::from(1)),
            ("type", sv("audio".into())),
        ]);
        let tracker_param = obj(vec![
            ("all_m", Value::from(1)),
            ("auth", sv(String::new())),
            ("is_free_part", Value::from(if q_bool(&query, "free_part") { 1 } else { 0 })),
            ("key", sv(key)),
            ("module_id", Value::from(0)),
            ("need_climax", Value::from(1)),
            ("need_xcdn", Value::from(1)),
            ("open_time", sv(String::new())),
            ("pid", sv("411".into())),
            ("pidversion", sv("3001".into())),
            ("priv_vip_type", sv("6".into())),
            ("viptoken", sv(vip_token)),
        ]);
        let data_map = obj(vec![
            ("area_code", sv("1".into())),
            ("behavior", sv("play".into())),
            (
                "qualities",
                json!(["128", "320", "flac", "high", "multitrack", "viper_atmos", "viper_tape", "viper_clear"]),
            ),
            ("resource", Value::Object(resource)),
            ("token", sv(token)),
            ("tracker_param", Value::Object(tracker_param)),
            ("userid", sv(userid.to_string())),
            ("vip", vip_type),
        ]);

        // cookie: Object.assign({}, { dfid }, params?.cookie)
        let mut cookie: Map<String, Value> = Map::new();
        cookie.insert("dfid".into(), sv(dfid));
        for (k, v) in cookie_of(&query) {
            cookie.insert(k, v);
        }

        let opts = RequestOptions {
            base_url: Some("http://tracker.kugou.com".into()),
            url: "/v6/priv_url".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie,
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 7 模块（sheet / top） =================

/// `params?.userid || params?.cookie?.userid || 0` -> Value（提供则字符串，否则数字 0）
fn uid_or_num(q: &Value) -> Value {
    let a = q_str(q, "userid");
    if !a.is_empty() {
        return sv(a);
    }
    let c = ck_str(q, "userid");
    if !c.is_empty() {
        return sv(c);
    }
    Value::from(0)
}

/// 复刻 `Number(str)`：空串 -> 0，可整除 -> i64，否则 f64；非数字 -> Null
fn js_number(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::from(0);
    }
    match t.parse::<f64>() {
        Ok(f) if f.is_finite() => {
            if f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15 {
                Value::from(f as i64)
            } else {
                Value::from(f)
            }
        }
        _ => Value::Null,
    }
}

/// /sheet/collection —— 乐谱广场首页模块
fn m_sheet_collection(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("srcappid", Value::from(config::CONFIG.srcappid)),
            ("position", s_or_num(&query, "position", 2)),
        ]);
        let opts = RequestOptions {
            url: "/miniyueku/v1/opern_square/get_home_module_config".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Web,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /sheet/collection/detail —— 乐谱合集详情
fn m_sheet_collection_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("srcappid", Value::from(config::CONFIG.srcappid)),
            ("page", s_or_num(&query, "page", 1)),
            ("collection_id", sv(q_str(&query, "collection_id"))),
        ]);
        let opts = RequestOptions {
            url: "/miniyueku/v1/opern_square/collection_detail".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Web,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /sheet/detail —— 乐谱详情
fn m_sheet_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("id", sv(q_str(&query, "id"))),
            ("source", sv(q_str(&query, "source"))),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://miniyueku.kugou.com".into()),
            url: "/v1/opern/detail".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /sheet/hot —— 推荐乐谱
fn m_sheet_hot(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("srcappid", Value::from(config::CONFIG.srcappid)),
            ("opern_type", s_or_num(&query, "opern_type", 1)),
        ]);
        let opts = RequestOptions {
            url: "/miniyueku/v1/opern_square/get_home_hot_opern".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Web,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /sheet/list —— 乐谱列表
fn m_sheet_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("album_audio_id", sv(q_str(&query, "album_audio_id"))),
            ("opern_type", s_or_num(&query, "opern_type", 0)),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/miniyueku/v1/opern/list".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /top/album —— 推荐专辑
fn m_top_album(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("apiver", Value::from(config::CONFIG.apiver)),
            ("token", sv(token_of(&query))),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("withpriv", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/musicadservice/v1/mobile_newalbum_sp".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /top/card —— 热门好歌精选
fn m_top_card(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let _dfid = dfid_of(&query); // 与 Node 一致读取但未使用
        let fakem = "60f7ebf1f812edbac3c63a7310001701760f";
        let date_time = now_ms();
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("platform", sv("android".into())),
            ("clienttime", Value::from(date_time)),
            ("userid", uid_or_num(&query)),
            ("key", sv(sign_params_key(&date_time.to_string(), None, None))),
            ("fakem", sv(fakem.into())),
            ("area_code", Value::from(1)),
            ("mid", mid_of(&query)),
            ("uuid", sv("-".into())),
            ("client_playlist", Value::Array(vec![])),
            ("u_info", sv("a0c35cd40af564444b5584c2754dedec".into())),
        ]);
        let params = obj(vec![
            ("card_id", s_or_num(&query, "card_id", 1)),
            ("fakem", sv(fakem.into())),
            ("area_code", Value::from(1)),
            ("platform", sv("ios".into())),
        ]);
        let opts = RequestOptions {
            url: "/singlecardrec.service/v1/single_card_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /top/card/youth —— 概念版热门好歌精选
fn m_top_card_youth(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("tagid", sv(q_str(&query, "tagid"))),
            ("u_info", sv(String::new())),
            ("source_mixsong", sv(String::new())),
        ]);
        let params = obj(vec![
            ("card_id", s_or_num(&query, "card_id", 3005)),
            ("area_code", Value::from(1)),
            ("platform", sv("ops".into())),
            ("module_id", Value::from(1)),
            ("ver", sv("v2".into())),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            // 注意：Node 源码 url 无前导斜杠，保持一致
            url: "youth/v1/song/single_card_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /top/ip —— 每日推荐（含 ip_id 提取后处理）
fn m_top_ip(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![("tags", Value::Object(Map::new()))]);
        let params = obj(vec![
            ("clientver", Value::from(12349)),
            ("area_code", Value::from(1)),
        ]);
        let opts = RequestOptions {
            base_url: Some("http://musicadservice.kugou.com".into()),
            url: "/v1/daily_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;

        // resp.body.status == 1（宽松等于）
        let status_1 = match res.body.get("status") {
            Some(Value::Number(n)) => n.as_f64() == Some(1.0),
            Some(Value::String(s)) => s == "1",
            _ => false,
        };
        if status_1 {
            if let Some(list) = res
                .body
                .pointer_mut("/data/list")
                .and_then(|v| v.as_array_mut())
            {
                for item in list.iter_mut() {
                    let inner = item
                        .pointer("/extra/inner_url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(inner) = inner {
                        if let Some(idx) = inner.rfind("ip_id") {
                            let sub = if inner.len() >= idx + 6 {
                                &inner[idx + 6..]
                            } else {
                                ""
                            };
                            let num = js_number(sub);
                            if let Some(extra) =
                                item.pointer_mut("/extra").and_then(|v| v.as_object_mut())
                            {
                                extra.insert("ip_id".into(), num);
                            }
                        }
                    }
                }
            }
        }
        Ok(res)
    })
}

/// /top/playlist —— 推荐歌单
fn m_top_playlist(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let date_time = now_sec().to_string(); // (Date.now()/1000).toFixed(0)
        let special_recommend = obj(vec![
            ("withtag", s_or_num(&query, "withtag", 1)),
            ("withsong", s_or_num(&query, "withsong", 1)),
            ("sort", s_or_num(&query, "sort", 1)),
            ("ugc", Value::from(1)),
            ("is_selected", Value::from(0)),
            ("withrecommend", Value::from(1)),
            ("area_code", Value::from(1)),
            ("categoryid", s_or_num(&query, "category_id", 0)),
        ]);
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("mid", mid_of(&query)),
            ("clientver", Value::from(use_clientver())),
            ("platform", sv("android".into())),
            ("clienttime", sv(date_time.clone())),
            ("userid", uid_or_num(&query)),
            ("module_id", s_or_num(&query, "module_id", 1)),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("key", sv(sign_params_key(&date_time, None, None))),
            ("special_recommend", Value::Object(special_recommend)),
            ("req_multi", Value::from(1)),
            ("retrun_min", Value::from(5)),
            ("return_special_falg", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/v2/special_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "specialrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /top/song —— 新歌速递
fn m_top_song(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("rank_id", s_or_num(&query, "type", 21608)),
            ("userid", uid_or_num(&query)),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("tags", Value::Array(vec![])),
        ]);
        let opts = RequestOptions {
            url: "/musicadservice/container/v1/newsong_publish".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 8 模块（scene / yueku / theme） =================

/// `params?.userid || params?.cookie?.userid || '0'` -> Value::String（缺省字符串 "0"）
fn uid_or_str0(q: &Value) -> Value {
    let a = q_str(q, "userid");
    if !a.is_empty() {
        return sv(a);
    }
    let c = ck_str(q, "userid");
    if !c.is_empty() {
        return sv(c);
    }
    sv("0".into())
}

/// /scene/audio/list —— 场景音频列表
fn m_scene_audio_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("token", sv(token_of(&query))),
            ("userid", uid_or_num(&query)),
        ]);
        let params = obj(vec![
            ("scene_id", sv(q_str(&query, "id"))),
            ("module_id", sv(q_str(&query, "module_id"))),
            ("tag", sv(q_str(&query, "tag"))),
            ("page", s_or_num(&query, "page", 1)),
            ("page_size", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/scene/v1/scene/audio_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/collection/list —— 场景合集列表
fn m_scene_collection_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("token", sv(token_of(&query))),
            ("userid", uid_or_num(&query)),
            ("tag_id", sv(q_str(&query, "tag_id"))),
            ("page", s_or_num(&query, "page", 1)),
            ("page_size", s_or_num(&query, "pagesize", 30)),
            ("exposed_data", Value::Array(vec![])),
        ]);
        let opts = RequestOptions {
            url: "/scene/v1/distribution/collection_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/lists —— 场景列表
fn m_scene_lists(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/scene/v1/scene/list".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/lists/v2 —— 场景列表 v2
fn m_scene_lists_v2(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // sortType[params.sort || 'rec'] || 1
        let sort_raw = q_str(&query, "sort");
        let sort_key = if sort_raw.is_empty() { "rec" } else { sort_raw.as_str() };
        let sort_type = match sort_key {
            "rec" => 1,
            "hot" => 2,
            "new" => 3,
            _ => 1,
        };
        let params = obj(vec![
            ("scene_id", sv(q_str(&query, "id"))),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("sort_type", Value::from(sort_type)),
            ("kugouid", uid_or_str0(&query)),
        ]);
        let data_map = obj(vec![("exposure", Value::Array(vec![]))]);
        let opts = RequestOptions {
            url: "/scene/v1/scene/list_v2".into(),
            method: "POST".into(),
            params,
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/module —— 场景模块
fn m_scene_module(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![("scene_id", sv(q_str(&query, "id")))]);
        let opts = RequestOptions {
            url: "/scene/v1/scene/module".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/module/info —— 场景模块详情
fn m_scene_module_info(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("scene_id", sv(q_str(&query, "id"))),
            ("module_id", sv(q_str(&query, "module_id"))),
        ]);
        let opts = RequestOptions {
            url: "/scene/v1/scene/module_info".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/music —— 场景推荐音乐
fn m_scene_music(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("scene_id", sv(q_str(&query, "id"))),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let data_map = obj(vec![("exposure", Value::Array(vec![]))]);
        let opts = RequestOptions {
            url: "/genesisapi/v1/scene_music/rec_music".into(),
            method: "POST".into(),
            params,
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /scene/video/list —— 场景视频列表
fn m_scene_video_list(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("token", sv(token_of(&query))),
            ("userid", uid_or_num(&query)),
            ("tag_id", sv(q_str(&query, "tag_id"))),
            ("page", s_or_num(&query, "page", 1)),
            ("page_size", s_or_num(&query, "pagesize", 30)),
            ("exposed_data", Value::Array(vec![])),
        ]);
        let opts = RequestOptions {
            url: "/scene/v1/distribution/video_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /yueku —— 安卓乐库推荐
fn m_yueku(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("operator", Value::from(7)),
            ("plat", Value::from(0)),
            ("type", Value::from(11)),
            ("area_code", Value::from(1)),
            ("req_multi", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/v1/yueku/recommend_v2".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "service.mobile.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /yueku/banner —— 乐库 banner
fn m_yueku_banner(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("plat", Value::from(0)),
            ("channel", Value::from(201)),
            ("operator", Value::from(7)),
            ("networktype", Value::from(2)),
            ("userid", uid_or_num(&query)),
            ("vip_type", Value::from(0)),
            ("m_type", Value::from(0)),
            ("tags", Value::Array(vec![])),
            ("apiver", Value::from(5)),
            ("ability", Value::from(2)),
            ("mode", sv("normal".into())),
        ]);
        let opts = RequestOptions {
            url: "/ads.gateway/v3/listen_banner".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /yueku/fm —— 乐库 fm
fn m_yueku_fm(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("operator", Value::from(7)),
            ("plat", Value::from(0)),
            ("type", Value::from(11)),
            ("area_code", Value::from(1)),
            ("req_multi", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/v1/time_fm_info".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "fm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /theme/music —— 主题音乐
fn m_theme_music(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("platform", sv("android".into())),
            ("clienttime", Value::from(now_sec())),
            ("show_theme_category_ids", sv(q_str(&query, "ids"))),
            ("userid", uid_or_num(&query)),
            ("module_id", Value::from(508)),
        ]);
        let opts = RequestOptions {
            url: "/everydayrec.service/v1/mul_theme_category_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /theme/music/detail —— 主题音乐详情
fn m_theme_music_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("platform", sv("android".into())),
            ("clienttime", Value::from(now_sec())),
            ("theme_category_id", sv(q_str(&query, "id"))),
            ("show_theme_category_id", Value::from(0)),
            ("userid", uid_or_num(&query)),
            ("module_id", Value::from(508)),
        ]);
        let opts = RequestOptions {
            url: "/everydayrec.service/v1/theme_category_recommend".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /theme/playlist —— 主题歌单
fn m_theme_playlist(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("platform", sv("android".into())),
            ("clientver", Value::from(use_clientver())),
            ("clienttime", Value::from(now_ms())),
            ("area_code", Value::from(1)),
            ("module_id", Value::from(1)),
            ("userid", uid_or_num(&query)),
        ]);
        let opts = RequestOptions {
            url: "/v2/getthemelist".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "everydayrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /theme/playlist/track —— 主题歌单歌曲
fn m_theme_playlist_track(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("platform", sv("android".into())),
            ("clientver", Value::from(use_clientver())),
            ("clienttime", Value::from(now_ms())),
            ("area_code", Value::from(1)),
            ("module_id", Value::from(1)),
            ("userid", uid_or_num(&query)),
            ("theme_id", sv(q_str(&query, "theme_id"))),
        ]);
        let opts = RequestOptions {
            url: "/v2/gettheme_songidlist".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "everydayrec.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 9 模块（youth 系列） =================

/// `params?.vip_type || params?.cookie?.vip_type || 0`（params 优先）
fn vip_type_pv(q: &Value) -> Value {
    let a = q_str(q, "vip_type");
    if !a.is_empty() {
        return sv(a);
    }
    let c = ck_str(q, "vip_type");
    if !c.is_empty() {
        return sv(c);
    }
    Value::from(0)
}

/// /youth/channel/all
fn m_youth_channel_all(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("type", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/youth/v2/channel/channel_all_list".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/amway
fn m_youth_channel_amway(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![("global_collection_id", sv(q_str(&query, "global_collection_id")))]);
        let opts = RequestOptions {
            url: "/youth/api/amway/v2/index".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/detail
fn m_youth_channel_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data: Vec<Value> = q_str(&query, "global_collection_id")
            .split(',')
            .map(|s| json!({ "global_collection_id": s }))
            .collect();
        let data_map = obj(vec![("data", Value::Array(data))]);
        let opts = RequestOptions {
            url: "/youth/api/channel/v1/channel_list_by_id".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/similar
fn m_youth_channel_similar(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("area_code", Value::from(1)),
            ("playlist_ver", Value::from(2)),
            ("vip_type", vip_type_pv(&query)),
            ("platform", sv("ios".into())),
        ]);
        let params = obj(vec![("channel_id", sv(q_str(&query, "channel_id")))]);
        let opts = RequestOptions {
            url: "/youth/v1/channel/get_friendly_channel".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/song
fn m_youth_channel_song(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("global_collection_id", sv(q_str(&query, "global_collection_id"))),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
            ("is_filter", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/youth/api/channel/v1/channel_get_song_audit_passed".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/song/detail
fn m_youth_channel_song_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("global_collection_id", sv(q_str(&query, "global_collection_id"))),
            ("fileid", sv(q_str(&query, "fileid"))),
        ]);
        let opts = RequestOptions {
            url: "/youth/v2/post/get_song_detail".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/channel/sub —— 订阅/取消订阅
fn m_youth_channel_sub(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // t = Number(params.t) === 0 ? 0 : 1（缺省 -> NaN -> 1）
        let t = match query.get("t").and_then(|v| v.as_str()) {
            None => 1,
            Some(s) => if js_number(s) == Value::from(0) { 0 } else { 1 },
        };
        let url = format!("/youth/v1/channel{}_subscribe", if t == 0 { "_un" } else { "" });
        let method = if t == 0 { "DELETE" } else { "POST" };
        let params = obj(vec![
            ("global_collection_id", sv(q_str(&query, "global_collection_id"))),
            ("source", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url,
            method: method.into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/day/vip —— 领取一天 VIP
fn m_youth_day_vip(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("source_id", Value::from(90139)),
            ("receive_day", sv(q_str(&query, "receive_day"))),
        ]);
        let opts = RequestOptions {
            url: "/youth/v1/recharge/receive_vip_listen_song".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/day/vip/upgrade —— 升级 VIP
fn m_youth_day_vip_upgrade(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("kugouid", Value::from(userid_of(&query))),
            ("ad_type", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/youth/v1/listen_song/upgrade_vip_reward".into(),
            method: "POST".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/dynamic
fn m_youth_dynamic(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/youth/v3/user/get_dynamic".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/dynamic/recent
fn m_youth_dynamic_recent(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            url: "/youth/v3/user/recent_dynamic".into(),
            method: "GET".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/listen/song —— 听歌领取 VIP（注意 Node 源码 encryptTyPe 拼写 -> 默认 android）
fn m_youth_listen_song(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![("mixsongid", s_or_num(&query, "mixsongid", 666075191))]);
        let params = obj(vec![("clientver", Value::from(10566))]);
        let opts = RequestOptions {
            url: "/youth/v2/report/listen_song".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android, // encryptTyPe 拼写 -> switch default = android
            cookie: cookie_of(&query),
            headers: str_map(&[
                ("user-agent", "Android13-1070-10566-201-0-ReportPlaySongToServerProtocol-wifi"),
                ("content-type", "application/json; charset=utf-8"),
            ]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/month/vip/record
fn m_youth_month_vip_record(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![("latest_limit", Value::from(100))]);
        let opts = RequestOptions {
            url: "/youth/v1/activity/get_month_vip_record".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/union/vip —— 领取联合 VIP
fn m_youth_union_vip(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("busi_type", sv("concept".into())),
            ("opt_product_types", sv("dvip,qvip".into())),
            ("product_type", sv("svip".into())),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://kugouvip.kugou.com".into()),
            url: "/v1/get_union_vip".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/user/song —— 用户公开歌曲
fn m_youth_user_song(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("filter_video", Value::from(0)),
            ("type", s_or_num(&query, "type", 0)),
            ("userid", sv(q_str(&query, "userid"))),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("page", s_or_num(&query, "page", 1)),
            ("is_filter", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/youth/v1/get_user_song_public".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /youth/vip —— 广告播放上报领 VIP
fn m_youth_vip(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let time = now_ms();
        let data_map = obj(vec![
            ("ad_id", Value::from(12307537187i64)),
            ("play_end", Value::from(time)),
            ("play_start", Value::from(time - 30000)),
        ]);
        let opts = RequestOptions {
            url: "/youth/v1/ad/play_report".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 10 模块（playlist 系列） =================

/// /playlist/add —— 收藏歌单
/// 注意：Node 里 `params.type === 0` / `params.source === 0` 为严格比较，
/// 而 req.query 恒为字符串，故这些数值严格比较【永远为 false】：
///   - is_pri 恒为 0（条件分支不触发）
///   - params 恒为 {}（三元恒走 else）
fn m_playlist_add(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("userid", uid_or_num(&query)),
            ("token", sv(token_of(&query))),
            ("total_ver", Value::from(0)),
            ("name", sv(q_str(&query, "name"))),
            ("type", s_or_num(&query, "type", 0)),
            ("source", s_or_num(&query, "source", 1)),
            ("is_pri", Value::from(0)),
            ("list_create_userid", sv(q_str(&query, "list_create_userid"))),
            ("list_create_listid", sv(q_str(&query, "list_create_listid"))),
            ("list_create_gid", sv(q_str(&query, "list_create_gid"))),
            ("from_shupinmv", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/cloudlist.service/v5/add_list".into(),
            data: Some(Value::Object(data_map)),
            params: Map::new(), // params.type === 0 恒 false -> {}
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/del —— 取消收藏歌单（arraybuffer + playlist AES/RSA）
fn m_playlist_del(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_num(&query);
        let token = token_of(&query);
        let clienttime = now_sec();

        let data_map = obj(vec![
            ("listid", n_of(&query, "listid")),
            ("total_ver", Value::from(0)),
            ("type", Value::from(1)),
        ]);
        let aes = crate::crypto::playlist_aes_encrypt(&json_str(&Value::Object(data_map)));

        // rsaEncrypt2({ aes: aes.key, uid: userid, token }).toUpperCase()
        let rsa_obj = json!({ "aes": aes.key, "uid": userid, "token": token });
        let p = rsa_encrypt2(&json_str(&rsa_obj)).to_uppercase();

        let params = obj(vec![
            ("clienttime", Value::from(clienttime)),
            ("key", sv(sign_params_key(&clienttime.to_string(), None, None))),
            ("last_area", sv("gztx".into())),
            ("clientver", Value::from(use_clientver())),
            ("appid", Value::from(use_appid())),
            ("last_time", Value::from(clienttime)),
            ("p", sv(p)),
        ]);

        let opts = RequestOptions {
            url: "/v2/delete_list".into(),
            params,
            data: Some(sv(aes.str.clone())),
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            headers: str_map(&[("x-router", "cloudlist.service.kugou.com")]),
            response_type: ResponseType::ArrayBuffer,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;
        // respone.body = playlistAesDecrypt({ str: body.toString('base64'), key: aes.key })
        let b64 = res.body.as_str().unwrap_or("").to_string();
        res.body = playlist_aes_decrypt(&aes.key, &b64);
        Ok(res)
    })
}

/// /playlist/detail —— 歌单详情
fn m_playlist_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data: Vec<Value> = q_str(&query, "ids")
            .split(',')
            .map(|s| json!({ "global_collection_id": s }))
            .collect();
        let data_map = obj(vec![
            ("data", Value::Array(data)),
            ("userid", uid_or_num(&query)),
            ("token", sv(token_of(&query))),
        ]);
        let opts = RequestOptions {
            url: "/v3/get_list_info".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "pubsongs.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/effect —— 音效歌单
fn m_playlist_effect(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let opts = RequestOptions {
            url: "/pubsongs/v1/get_sound_effect_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/similar —— 相似歌单
fn m_playlist_similar(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data: Vec<Value> = q_str(&query, "ids")
            .split(',')
            .map(|s| json!({ "global_collection_id": s }))
            .collect();
        let clienttime = now_ms();
        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("clientver", Value::from(use_clientver())),
            ("clienttime", Value::from(clienttime)),
            ("key", sv(sign_params_key(&clienttime.to_string(), None, None))),
            ("userid", uid_or_num(&query)),
            ("ugc", Value::from(1)),
            ("show_list", Value::from(1)),
            ("need_songs", Value::from(1)),
            ("data", Value::Array(data)),
        ]);
        let opts = RequestOptions {
            url: "/pubsongs/v1/kmr_get_similar_lists".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/tags —— 歌单分类
fn m_playlist_tags(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let data_map = obj(vec![
            ("tag_type", sv("collection".into())),
            ("tag_id", Value::from(0)),
            ("source", Value::from(3)),
        ]);
        let opts = RequestOptions {
            url: "/pubsongs/v1/get_tags_by_type".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/track/all —— 歌单所有歌曲
fn m_playlist_track_all(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // pagesize = params?.pagesize || 30（提供则原样字符串）
        let pagesize_val = s_or_num(&query, "pagesize", 30);
        let pagesize_num = {
            let p = q_str(&query, "pagesize");
            if p.is_empty() { 30.0 } else { p.parse::<f64>().unwrap_or(f64::NAN) }
        };
        // begin_idx = (Number(params.page || 1) - 1) * pagesize
        let page_num = {
            let p = q_str(&query, "page");
            if p.is_empty() { 1.0 } else { p.parse::<f64>().unwrap_or(f64::NAN) }
        };
        let begin_idx = (page_num - 1.0) * pagesize_num;
        let begin_idx_val = if begin_idx.is_finite() && begin_idx.fract() == 0.0 {
            Value::from(begin_idx as i64)
        } else if begin_idx.is_finite() {
            Value::from(begin_idx)
        } else {
            Value::Null
        };
        let params = obj(vec![
            ("area_code", Value::from(1)),
            ("begin_idx", begin_idx_val),
            ("plat", Value::from(1)),
            ("type", Value::from(1)),
            ("mode", Value::from(1)),
            ("personal_switch", Value::from(1)),
            ("extend_fields", sv("abtags,hot_cmt,popularization".into())),
            ("pagesize", pagesize_val),
            ("global_collection_id", sv(q_str(&query, "id"))),
        ]);
        let opts = RequestOptions {
            url: "/pubsongs/v2/get_other_list_file_nofilt".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/track/all/new —— 歌单所有歌曲（v4）
fn m_playlist_track_all_new(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // token: params?.token || cookie?.token || '0'
        let token = {
            let t = token_of(&query);
            if t.is_empty() { "0".to_string() } else { t }
        };
        let data_map = obj(vec![
            ("listid", sv(q_str(&query, "listid"))),
            ("userid", uid_or_str0(&query)),
            ("area_code", Value::from(1)),
            ("show_relate_goods", Value::from(0)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
            ("allplatform", Value::from(1)),
            ("show_cover", Value::from(1)),
            ("type", Value::from(0)),
            ("token", sv(token)),
            ("page", s_or_num(&query, "page", 1)),
        ]);
        let opts = RequestOptions {
            url: "/v4/get_list_all_file".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "cloudlist.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/tracks/add —— 歌单添加歌曲
fn m_playlist_tracks_add(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let clienttime = now_sec();
        // resource = (params.data || '').split(',').map(s => {...})
        let resource: Vec<Value> = q_str(&query, "data")
            .split(',')
            .map(|s| {
                let d: Vec<&str> = s.split('|').collect();
                let g = |i: usize| d.get(i).copied().unwrap_or("");
                json!({
                    "number": 1,
                    "name": g(0),
                    "hash": g(1),
                    "size": 0,
                    "sort": 0,
                    "timelen": 0,
                    "bitrate": 0,
                    "album_id": js_number(g(2)),
                    "mixsongid": js_number(g(3)),
                })
            })
            .collect();
        let data_map = obj(vec![
            ("userid", uid_or_num(&query)),
            ("token", sv(token_of(&query))),
            ("listid", sv(q_str(&query, "listid"))),
            ("list_ver", Value::from(0)),
            ("type", Value::from(0)),
            ("slow_upload", Value::from(1)),
            ("scene", sv("false;null".into())),
            ("data", Value::Array(resource)),
        ]);
        let params = obj(vec![
            ("last_time", Value::from(clienttime)),
            ("last_area", sv("gztx".into())),
            ("userid", uid_or_num(&query)),
            ("token", sv(token_of(&query))),
        ]);
        let opts = RequestOptions {
            url: "/cloudlist.service/v6/add_song".into(),
            data: Some(Value::Object(data_map)),
            params,
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playlist/tracks/del —— 歌单删除歌曲
fn m_playlist_tracks_del(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let resource: Vec<Value> = q_str(&query, "fileids")
            .split(',')
            .map(|s| json!({ "fileid": js_number(s) }))
            .collect();
        let data_map = obj(vec![
            ("listid", sv(q_str(&query, "listid"))),
            ("userid", uid_or_num(&query)),
            ("data", Value::Array(resource)),
            ("type", Value::from(0)),
            ("token", sv(token_of(&query))),
            ("list_ver", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/v4/delete_songs".into(),
            data: Some(Value::Object(data_map)),
            method: "POST".into(),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "cloudlist.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

// ================= 批次 11 模块（user / video / 其余收尾） =================

/// `params?.x ?? default`（nullish 合并：仅缺失/null 时用默认值，否则保留原始类型）
fn nn_or(q: &Value, key: &str, default: Value) -> Value {
    match q_get(q, key) {
        Some(v) if !v.is_null() => v.clone(),
        _ => default,
    }
}

/// `params?.userid || params?.cookie?.userid || '0'` 的首个非空字符串（用于模板拼接）
fn uid_str_of(q: &Value) -> String {
    let a = q_str(q, "userid");
    if !a.is_empty() {
        return a;
    }
    let c = ck_str(q, "userid");
    if !c.is_empty() {
        return c;
    }
    "0".to_string()
}

/// /user/detail —— 用户信息
fn m_user_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let token = token_of(&query);
        // Number(params?.userid || cookie?.userid || '0')
        let userid = js_number(&uid_str_of(&query));
        let clienttime_ms = now_sec(); // Node 变量名叫 ms 实为秒（原样复刻）
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "token": token, "clienttime": clienttime_ms })),
            None,
        )
        .to_uppercase();

        let data_map = obj(vec![
            ("visit_time", Value::from(clienttime_ms)),
            ("usertype", Value::from(1)),
            ("p", sv(pk)),
            ("userid", userid),
        ]);
        let opts = RequestOptions {
            url: "/v3/get_my_info".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params: obj(vec![("plat", Value::from(1))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "usercenter.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/follow —— 用户关注的歌手
fn m_user_follow(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let token = token_of(&query);
        let userid = uid_or_str0(&query);
        let date_time = now_sec();
        let p = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime": date_time, "token": token })),
            None,
        )
        .to_uppercase();

        let data_map = obj(vec![
            ("merge", Value::from(2)),
            ("need_iden_type", Value::from(1)),
            ("ext_params", sv("k_pic,jumptype,singerid,score".into())),
            ("userid", userid),
            ("type", Value::from(0)),
            ("id_type", Value::from(0)),
            ("p", sv(p)),
        ]);
        let opts = RequestOptions {
            url: "/v4/follow_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params: obj(vec![("plat", Value::from(1))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "relationuser.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/follow/message —— 关注歌手的消息
fn m_user_follow_message(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_str_of(&query);
        // tag: `chat:${userid}_${params.id}`（id 缺失 -> "undefined"）
        let id_s = match q_get(&query, "id") {
            Some(v) => crate::jsval::js_to_string(v),
            None => "undefined".to_string(),
        };
        let params = obj(vec![
            ("filter", Value::from(1)),
            ("maxid", Value::from(0)),
            ("pagesize", nn_or(&query, "pagesize", Value::from(30))),
            ("tag", sv(format!("chat:{}_{}", userid, id_s))),
        ]);
        let opts = RequestOptions {
            url: "/msg.mobile/v3/msgtag/history".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/history —— 用户听歌排行
fn m_user_history(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_num(&query);
        let token = token_of(&query);
        let mut data_map = obj(vec![
            ("token", sv(token)),
            ("userid", userid),
            ("source_classify", sv("app".into())),
            ("to_subdivide_sr", Value::from(1)),
        ]);
        // if (params.bp) dataMap['bp'] = params.bp
        if let Some(v) = q_get(&query, "bp").filter(|v| truthy(v)) {
            data_map.insert("bp".into(), v.clone());
        }
        let opts = RequestOptions {
            url: "/playhistory/v1/get_songs".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/listen —— 用户听书/听歌列表（listenservice）
fn m_user_listen(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let token = token_of(&query);
        let userid = uid_or_str0(&query);
        let clienttime = now_sec();
        let p = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime": clienttime, "token": token })),
            None,
        )
        .to_uppercase();

        let data_map = obj(vec![
            ("t_userid", userid.clone()),
            ("userid", userid),
            ("list_type", s_or_num(&query, "type", 0)),
            ("area_code", Value::from(1)),
            ("cover", Value::from(2)),
            ("p", sv(p)),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://listenservice.kugou.com".into()),
            url: "/v2/get_list".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params: obj(vec![
                ("clienttime", Value::from(clienttime)),
                ("plat", Value::from(0)),
            ]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/playlist —— 用户歌单（cookie 优先）
fn m_user_playlist(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // userid = cookie?.userid || params?.userid || 0（cookie 优先，保留原始类型）
        let userid = userid_ck_first_opt(&query).unwrap_or(Value::from(0));
        let token = token_ck_first_opt(&query).unwrap_or(sv(String::new()));

        let data_map = obj(vec![
            ("userid", userid.clone()),
            ("token", token.clone()),
            ("total_ver", Value::from(979)),
            ("type", Value::from(2)),
            ("page", s_or_num(&query, "page", 1)),
            ("pagesize", s_or_num(&query, "pagesize", 30)),
        ]);
        let params = obj(vec![
            ("plat", Value::from(1)),
            // userid: Number(userid)
            ("userid", js_number(&crate::jsval::js_to_string(&userid))),
            ("token", token),
        ]);
        let opts = RequestOptions {
            url: "/v7/get_all_list".into(),
            method: "post".into(),
            data: Some(Value::Object(data_map)),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "cloudlist.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/video/collect —— 用户收藏的视频
fn m_user_video_collect(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let token = token_of(&query);
        let userid = uid_or_str0(&query);
        let data_map = obj(vec![
            ("userid", userid),
            ("token", sv(token)),
            ("page", nn_or(&query, "page", Value::from(1))),
            ("pagesize", nn_or(&query, "pagesize", Value::from(30))),
        ]);
        let opts = RequestOptions {
            url: "/collectservice/v2/collect_list_mixvideo".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params: obj(vec![("plat", Value::from(1))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/video/love —— 用户点赞的视频
fn m_user_video_love(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_str0(&query);
        let params = obj(vec![
            ("kugouid", userid),
            ("pagesize", nn_or(&query, "pagesize", Value::from(30))),
            ("load_video_info", Value::from(1)),
            ("p", Value::from(1)),
            ("plat", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/m.comment.service/v1/get_user_like_video".into(),
            method: "get".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/vip/detail —— 会员详情
fn m_user_vip_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let opts = RequestOptions {
            base_url: Some("https://kugouvip.kugou.com".into()),
            url: "/v1/get_union_vip".into(),
            method: "GET".into(),
            params: obj(vec![("busi_type", sv("concept".into()))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /user/cloud/url —— 云盘音乐 url
fn m_user_cloud_url(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // String(params.hash).toLocaleLowerCase()（缺失 -> "undefined"）
        let hash = match q_get(&query, "hash") {
            Some(v) => crate::jsval::js_to_string(v),
            None => "undefined".to_string(),
        }
        .to_lowercase();

        let params = obj(vec![
            ("hash", sv(hash.clone())),
            ("ssa_flag", sv("is_fromtrack".into())),
            ("version", sv("20102".into())),
            ("ssl", Value::from(0)),
            ("album_audio_id", nn_or(&query, "album_audio_id", Value::from(0))),
            ("pid", Value::from(20026)),
            ("audio_id", nn_or(&query, "audio_id", Value::from(0))),
            ("kv_id", Value::from(2)),
            ("key", sv(sign_cloud_key(&hash, "20026"))),
            ("bucket", sv("musicclound".into())),
            ("name", nn_or(&query, "name", sv(String::new()))),
            ("with_res_tag", Value::from(0)),
        ]);
        let opts = RequestOptions {
            url: "/bsstrackercdngz/v2/query_musicclound_url".into(),
            method: "get".into(),
            params,
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /video/detail —— 视频详情（clearDefaultParams + signParamsKey）
fn m_video_detail(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // dfid = params?.cookie?.dfid || '-'（仅 cookie，无 params 回退）
        let dfid = {
            let c = ck_str(&query, "dfid");
            if c.is_empty() { "-".to_string() } else { c }
        };
        let mid = ck_get(&query, "KUGOU_API_MID").cloned(); // undefined -> JSON.stringify 丢弃
        // uuid = md5(`${dfid}${mid}`)（mid 缺失 -> "undefined" 参与模板）
        let uuid = crypto_md5(&format!("{}{}", dfid, ck_tmpl(&query, "KUGOU_API_MID")));
        let token = token_of(&query);
        let clienttime = now_sec();

        let resource: Vec<Value> = q_str(&query, "id")
            .split(',')
            .map(|s| json!({ "video_id": s }))
            .collect();

        let mut data_map = Map::new();
        data_map.insert("appid".into(), Value::from(use_appid()));
        data_map.insert("clientver".into(), Value::from(use_clientver()));
        data_map.insert("clienttime".into(), Value::from(clienttime));
        if let Some(v) = mid {
            data_map.insert("mid".into(), v);
        }
        data_map.insert("uuid".into(), sv(uuid));
        data_map.insert("dfid".into(), sv(dfid));
        data_map.insert("token".into(), sv(token));
        data_map.insert(
            "key".into(),
            sv(sign_params_key(&clienttime.to_string(), None, None)),
        );
        data_map.insert("show_resolution".into(), Value::from(1));
        data_map.insert("data".into(), Value::Array(resource));

        let opts = RequestOptions {
            url: "/v1/video".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            clear_default_params: true,
            headers: str_map(&[("x-router", "kmr.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /video/privilege —— 视频特权
fn m_video_privilege(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let dfid = {
            let c = ck_str(&query, "dfid");
            if c.is_empty() { "-".to_string() } else { c }
        };
        let mid = ck_get(&query, "KUGOU_API_MID").cloned();

        let resource: Vec<Value> = q_str(&query, "hash")
            .split(',')
            .map(|s| json!({ "hash": s, "id": 0, "name": "" }))
            .collect();

        let mut data_map = Map::new();
        data_map.insert("appid".into(), Value::from(use_appid()));
        data_map.insert("area_code".into(), Value::from(1));
        data_map.insert("behavior".into(), sv("play".into()));
        data_map.insert("clientver".into(), Value::from(use_clientver()));
        data_map.insert("dfid".into(), sv(dfid));
        if let Some(v) = mid {
            data_map.insert("mid".into(), v);
        }
        data_map.insert("resource".into(), Value::Array(resource));
        // token: cookie?.token || ''；userid: cookie?.userid || 0；vip: cookie?.vip_type || 0
        data_map.insert("token".into(), sv(ck_str(&query, "token")));
        data_map.insert(
            "userid".into(),
            ck_get(&query, "userid").filter(|v| truthy(v)).cloned().unwrap_or(Value::from(0)),
        );
        data_map.insert(
            "vip".into(),
            ck_get(&query, "vip_type").filter(|v| truthy(v)).cloned().unwrap_or(Value::from(0)),
        );

        let opts = RequestOptions {
            url: "/v1/get_video_privilege".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "media.store.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /video/url —— 视频播放地址（encryptKey）
fn m_video_url(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let params = obj(vec![
            ("backupdomain", Value::from(1)),
            ("cmd", Value::from(123)),
            ("ext", sv("mp4".into())),
            ("ismp3", Value::from(0)),
            // hash: params.hash（缺失 -> undefined -> axios 跳过；Null 同样被跳过）
            ("hash", q_get(&query, "hash").cloned().unwrap_or(Value::Null)),
            ("pid", Value::from(1)),
            ("type", Value::from(1)),
        ]);
        let opts = RequestOptions {
            url: "/v2/interface/index".into(),
            method: "GET".into(),
            params,
            encrypt_type: EncryptType::Android,
            encrypt_key: true,
            // Node 版未传 cookie -> {}
            headers: str_map(&[("x-router", "trackermv.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /pc/diantai —— 电台 banner
fn m_pc_diantai(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // userid = cookie?.userid || params?.userid || 0（cookie 优先）
        let userid = userid_ck_first_opt(&query).unwrap_or(Value::from(0));
        let data_map = obj(vec![
            ("isvip", Value::from(0)),
            ("userid", userid),
            ("vipType", Value::from(0)),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://adservice.kugou.com".into()),
            url: "/v3/pc_diantai".into(),
            method: "post".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /personal/fm —— 私人 FM
fn m_personal_fm(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // userid/token/vip_type 均 cookie 优先
        let userid = userid_ck_first_opt(&query); // cookie?.userid || params?.userid || 0
        // token = cookie?.token || params?.token || 0（回退数字 0 为 falsy -> 不插入）
        let token = token_ck_first_opt(&query);
        // vip_type = cookie?.vip_type || params?.vipType || 0
        let vip_type = ck_get(&query, "vip_type")
            .filter(|v| truthy(v))
            .cloned()
            .or_else(|| q_get(&query, "vipType").filter(|v| truthy(v)).cloned());
        let date_time = now_ms();

        let mut data_map = Map::new();
        data_map.insert("appid".into(), Value::from(use_appid()));
        data_map.insert("clienttime".into(), Value::from(date_time));
        // mid: cookie?.KUGOU_API_MID（undefined -> JSON.stringify 丢弃）
        if let Some(v) = ck_get(&query, "KUGOU_API_MID").cloned() {
            data_map.insert("mid".into(), v);
        }
        data_map.insert("action".into(), sv(q_str_or(&query, "action", "play")));
        data_map.insert("recommend_source_locked".into(), Value::from(0));
        // Number(params?.song_pool_id || 0)
        data_map.insert("song_pool_id".into(), js_number(&q_str_or(&query, "song_pool_id", "0")));
        data_map.insert("callerid".into(), Value::from(0));
        data_map.insert("m_type".into(), Value::from(1));
        data_map.insert("platform".into(), sv(q_str_or(&query, "platform", "ios")));
        data_map.insert("area_code".into(), Value::from(1));
        data_map.insert("remain_songcnt".into(), js_number(&q_str_or(&query, "remain_songcnt", "0")));
        data_map.insert("clientver".into(), Value::from(use_clientver()));
        data_map.insert(
            "is_overplay".into(),
            Value::from(if q_get(&query, "is_overplay").map(truthy).unwrap_or(false) { 1 } else { 0 }),
        );
        data_map.insert("mode".into(), sv(q_str_or(&query, "mode", "normal")));
        data_map.insert("fakem".into(), sv("ca981cfc583a4c37f28d2d49000013c16a0a".into()));
        data_map.insert("key".into(), sv(sign_params_key(&date_time.to_string(), None, None)));

        if let Some(uid) = userid {
            data_map.insert("userid".into(), uid.clone());
            data_map.insert("kguid".into(), uid);
        }
        if let Some(t) = token {
            data_map.insert("token".into(), t);
        }
        if let Some(v) = vip_type {
            data_map.insert("vip_type".into(), v);
        }
        for k in ["hash", "songid", "playtime"] {
            if let Some(v) = q_get(&query, k).filter(|v| truthy(v)) {
                data_map.insert(k.to_string(), v.clone());
            }
        }

        let opts = RequestOptions {
            url: "/v2/personal_recommend".into(),
            method: "post".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[("x-router", "persnfm.service.kugou.com")]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /playhistory/upload —— 提交听歌历史
fn m_playhistory_upload(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_num(&query);
        let token = token_of(&query);
        // ot: Number(params.time || nowSec)
        let ot = {
            let t = q_str(&query, "time");
            if t.is_empty() { Value::from(now_sec()) } else { js_number(&t) }
        };
        let song = obj(vec![
            ("mxid", js_number(&q_str(&query, "mxid"))),
            ("op", Value::from(1)),
            ("ot", ot),
            ("pc", js_number(&q_str_or(&query, "pc", "1"))),
        ]);
        let data_map = obj(vec![
            ("songs", Value::Array(vec![Value::Object(song)])),
            ("token", sv(token)),
            ("userid", userid),
        ]);
        let opts = RequestOptions {
            url: "/playhistory/v1/upload_songs".into(),
            method: "POST".into(),
            data: Some(Value::Object(data_map)),
            params: obj(vec![("plat", Value::from(3))]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// /privilege/lite —— 概念版歌曲特权
fn m_privilege_lite(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let mut resource: Vec<Value> = q_str(&query, "hash")
            .split(',')
            .map(|s| json!({ "type": "audio", "page_id": 0, "hash": s, "album_id": 0 }))
            .collect();
        // (params?.album_id || '').split(',').forEach((s,l)=> resource[l].album_id = s)
        // 注意：album_id 缺省时 ''.split(',') -> ['']，会把 resource[0].album_id 置为 ""（原样复刻）
        for (l, s) in q_str(&query, "album_id").split(',').enumerate() {
            if let Some(item) = resource.get_mut(l) {
                item["album_id"] = sv(s.to_string());
            }
        }

        let data_map = obj(vec![
            ("appid", Value::from(use_appid())),
            ("area_code", Value::from(1)),
            ("behavior", sv("play".into())),
            ("clientver", Value::from(use_clientver())),
            ("need_hash_offset", Value::from(1)),
            ("relate", Value::from(1)),
            ("support_verify", Value::from(1)),
            ("resource", Value::Array(resource)),
            (
                "qualities",
                json!(["128", "320", "flac", "high", "viper_atmos", "viper_tape", "viper_clear"]),
            ),
        ]);
        let opts = RequestOptions {
            url: "/v2/get_res_privilege/lite".into(),
            method: "post".into(),
            data: Some(Value::Object(data_map)),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            headers: str_map(&[
                ("x-router", "media.store.kugou.com"),
                ("Content-Type", "application/json"),
            ]),
            ..Default::default()
        };
        use_axios(opts).await
    })
}

/// `params?.x || default`（保留原始类型；falsy 时用默认 Value）
fn or_val(q: &Value, key: &str, default: Value) -> Value {
    match q_get(q, key).filter(|v| truthy(v)) {
        Some(v) => v.clone(),
        None => default,
    }
}

/// /user/cloud —— 云盘列表（arraybuffer + playlist AES/RSA + notSignature）
fn m_user_cloud(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_num(&query);
        let token = token_of(&query);
        let mid = mid_of(&query); // Null -> axios 跳过
        let clienttime = now_sec();

        let data_map = obj(vec![
            ("page", nn_or(&query, "page", Value::from(1))),
            ("pagesize", nn_or(&query, "pagesize", Value::from(30))),
            ("getkmr", Value::from(1)),
        ]);
        let aes = playlist_aes_encrypt(&json_str(&Value::Object(data_map)));
        let p = rsa_encrypt2(&json_str(&json!({ "aes": aes.key, "uid": userid, "token": token })))
            .to_uppercase();

        let params = obj(vec![
            ("clienttime", Value::from(clienttime)),
            ("mid", mid),
            ("key", sv(sign_params_key(&clienttime.to_string(), Some(use_appid()), None))),
            ("clientver", Value::from(use_clientver())),
            ("appid", Value::from(use_appid())),
            ("p", sv(p)),
        ]);
        let opts = RequestOptions {
            base_url: Some("https://mcloudservice.kugou.com".into()),
            url: "/v1/get_list".into(),
            method: "post".into(),
            params,
            // data: Buffer.from(aesEncrypt.str, 'base64') —— 原始二进制 body
            data_binary: Some(B64.decode(&aes.str).unwrap_or_default()),
            encrypt_type: EncryptType::Android,
            response_type: ResponseType::ArrayBuffer,
            cookie: cookie_of(&query),
            clear_default_params: true,
            not_signature: true,
            ..Default::default()
        };
        // Node 版整体 try/catch：任何失败 resolve({status:500, body: error})
        match use_axios(opts).await {
            Ok(mut res) => {
                let b64 = res.body.as_str().unwrap_or("").to_string();
                res.body = playlist_aes_decrypt(&aes.key, &b64);
                Ok(res)
            }
            Err(e) => Ok(Response {
                status: 500,
                body: json!({
                    "status": e.status,
                    "body": e.body,
                    "cookie": e.cookie,
                    "headers": e.headers,
                }),
                cookie: vec![],
                headers: Default::default(),
            }),
        }
    })
}

/// /register/dev —— 设备注册（arraybuffer + playlist AES/RSA）
fn m_register_dev(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let userid = uid_or_num(&query);
        let token = token_of(&query);

        let mut data_map = Map::new();
        data_map.insert("availableRamSize".into(), or_val(&query, "availableRamSize", Value::from(4983533568i64)));
        data_map.insert("availableRomSize".into(), or_val(&query, "availableRomSize", Value::from(48114719)));
        data_map.insert("availableSDSize".into(), or_val(&query, "availableSDSize", Value::from(48114717)));
        data_map.insert("basebandVer".into(), or_val(&query, "basebandVer", sv(String::new())));
        data_map.insert("batteryLevel".into(), or_val(&query, "batteryLevel", Value::from(100)));
        data_map.insert("batteryStatus".into(), or_val(&query, "batteryStatus", Value::from(3)));
        data_map.insert("brand".into(), or_val(&query, "brand", sv("Redmi".into())));
        data_map.insert("buildSerial".into(), or_val(&query, "buildSerial", sv("unknown".into())));
        data_map.insert("device".into(), or_val(&query, "device", sv("marble".into())));
        // imei/uuid: `params?.x || cookie?.KUGOU_API_GUID`（两者皆缺 -> undefined -> JSON 丢弃）
        let guid_fallback = |field: &str| {
            q_get(&query, field)
                .filter(|v| truthy(v))
                .cloned()
                .or_else(|| ck_get(&query, "KUGOU_API_GUID").cloned())
        };
        if let Some(v) = guid_fallback("imei") {
            data_map.insert("imei".into(), v);
        }
        data_map.insert("imsi".into(), or_val(&query, "imsi", sv(String::new())));
        data_map.insert("manufacturer".into(), or_val(&query, "manufacturer", sv("Xiaomi".into())));
        if let Some(v) = guid_fallback("uuid") {
            data_map.insert("uuid".into(), v);
        }
        for sensor in [
            "accelerometer", "gravity", "gyroscope", "light", "magnetic",
            "orientation", "pressure", "step_counter", "temperature",
        ] {
            data_map.insert(sensor.to_string(), or_val(&query, sensor, Value::from(false)));
            let vk = format!("{}Value", sensor);
            data_map.insert(vk.clone(), or_val(&query, &vk, sv(String::new())));
        }

        let aes = playlist_aes_encrypt(&json_str(&Value::Object(data_map)));
        // p 不做 toUpperCase（与 user_cloud 不同，原样复刻）
        let p = rsa_encrypt2(&json_str(&json!({ "aes": aes.key, "uid": userid, "token": token })));

        let opts = RequestOptions {
            base_url: Some("https://userservice.kugou.com".into()),
            url: "/risk/v2/r_register_dev".into(),
            method: "POST".into(),
            // data: aesEncrypt.str（base64 字符串直接作为 body，参与签名）
            data: Some(sv(aes.str.clone())),
            params: obj(vec![
                ("part", Value::from(1)),
                ("platid", Value::from(1)),
                ("p", sv(p)),
            ]),
            encrypt_type: EncryptType::Android,
            cookie: cookie_of(&query),
            response_type: ResponseType::ArrayBuffer,
            ..Default::default()
        };
        let mut res = use_axios(opts).await?;
        let b64 = res.body.as_str().unwrap_or("").to_string();
        res.body = playlist_aes_decrypt(&aes.key, &b64);

        // if (body?.status === 1 && body?.data) cookie.push(`dfid=${body.data['dfid']}`)
        if res.body.get("status").and_then(|v| v.as_i64()) == Some(1)
            && res.body.get("data").map(truthy).unwrap_or(false)
        {
            res.cookie.push(format!("dfid={}", data_tmpl(&res, "dfid")));
        }
        Ok(res)
    })
}

// ---------------- 微信直连辅助（对应模块内裸 axios 调用） ----------------

static DIRECT_CLIENT: once_cell::sync::Lazy<reqwest::Client> =
    once_cell::sync::Lazy::new(reqwest::Client::new);

/// 直连第三方 URL（axios 默认行为：尝试 JSON.parse，失败保留字符串；
/// 非 2xx 状态 reject —— 对应 axios 默认 validateStatus: 2xx only）
async fn direct_request(method: &str, url: &str, params: &[(String, String)]) -> Result<Value, String> {
    let req = if method.eq_ignore_ascii_case("post") {
        DIRECT_CLIENT.post(url)
    } else {
        DIRECT_CLIENT.get(url)
    };
    let req = if params.is_empty() { req } else { req.query(params) };
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        // AxiosError.message 形制
        return Err(format!("Request failed with status code {}", status.as_u16()));
    }
    Ok(serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
}

/// 502 信封：{status: 0, msg}
fn resp_502(msg: Value) -> Response {
    Response {
        status: 502,
        body: json!({ "status": 0, "msg": msg }),
        cookie: vec![],
        headers: Default::default(),
    }
}

/// /login/openplat —— 微信开放平台登录（直连 api.weixin.qq.com 换 token 后走网关）
fn m_login_openplat(query: Value, use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let is_lite = config::is_lite();
        let (wx_appid, wx_secret) = if is_lite {
            (config::CONFIG.wx_lite_appid.clone(), config::CONFIG.wx_lite_secret.clone())
        } else {
            (config::CONFIG.wx_appid.clone(), config::CONFIG.wx_secret.clone())
        };

        // assetsToken：POST /sns/oauth2/access_token（params 走 query string）
        let token_resp = direct_request(
            "POST",
            "https://api.weixin.qq.com/sns/oauth2/access_token",
            &[
                ("secret".into(), wx_secret),
                ("appid".into(), wx_appid),
                ("code".into(), q_str(&query, "code")),
                ("grant_type".into(), "authorization_code".into()),
            ],
        )
        .await
        .map_err(|e| resp_502(Value::String(e)))?;

        let has_token = token_resp.get("access_token").map(truthy).unwrap_or(false)
            && token_resp.get("openid").map(truthy).unwrap_or(false);
        if !has_token {
            return Err(resp_502(token_resp));
        }

        let date_now = now_ms();
        let access_token = token_resp["access_token"].clone();
        let openid = token_resp["openid"].clone();

        let encrypt = crypto_aes_encrypt(&json_str(&json!({ "access_token": access_token })), None, None);
        let pk = crypto_rsa_encrypt(
            &json_str(&json!({ "clienttime_ms": date_now, "key": encrypt.key })),
            None,
        )
        .to_uppercase();
        // t2 明文模板：GUID|固定串|MAC|DEV|now（缺失 cookie -> "undefined"）
        let t2_plain = format!(
            "{}|0f607264fc6318a92b9e13c65db7cd3c|{}|{}|{}",
            ck_tmpl(&query, "KUGOU_API_GUID"),
            ck_tmpl(&query, "KUGOU_API_MAC"),
            ck_tmpl(&query, "KUGOU_API_DEV"),
            date_now
        );
        let t2 = crypto_aes_encrypt(&t2_plain, Some(LITE_T2_KEY), Some(LITE_T2_IV)).str;
        let t1 = crypto_aes_encrypt(&format!("|{}", date_now), Some(LITE_T1_KEY), Some(LITE_T1_IV)).str;

        let mut data = Map::new();
        // dev: cookie?.KUGOU_API_DEV（undefined -> JSON.stringify 丢弃）
        if let Some(v) = ck_get(&query, "KUGOU_API_DEV").cloned() {
            data.insert("dev".into(), v);
        }
        data.insert("force_login".into(), Value::from(1));
        data.insert("partnerid".into(), Value::from(36));
        data.insert("clienttime_ms".into(), Value::from(date_now));
        data.insert("t1".into(), if is_lite { sv(t1) } else { Value::from(0) });
        data.insert("t2".into(), if is_lite { sv(t2) } else { Value::from(0) });
        data.insert("t3".into(), sv(T3_FIXED.into()));
        data.insert("openid".into(), openid);
        data.insert("params".into(), sv(encrypt.str.clone()));
        data.insert("pk".into(), sv(pk));

        let opts = RequestOptions {
            url: "/v6/login_by_openplat".into(),
            method: "POST".into(),
            data: Some(Value::Object(data)),
            cookie: cookie_of(&query),
            encrypt_type: EncryptType::Android,
            headers: str_map(&[("x-router", "login.user.kugou.com")]),
            ..Default::default()
        };
        let mut res = use_axios(opts).await.map_err(|e| {
            // Node 版 catch(error) -> 502 {status:0, msg:error}
            resp_502(json!({ "status": e.status, "body": e.body }))
        })?;

        if res.body.get("status").and_then(|v| v.as_i64()) == Some(1) {
            apply_secu(&mut res, &encrypt.key, true);
            // t1: `${data?.t1 ?? ''}`（nullish：仅 null/缺失 -> ''）
            let t1v = match res.body.pointer("/data/t1") {
                Some(Value::Null) | None => String::new(),
                Some(v) => crate::jsval::js_to_string(v),
            };
            res.cookie.push(format!("t1={}", t1v));
            res.cookie.push(format!("userid={}", data_or(&res, "userid", "0")));
            res.cookie.push(format!("vip_type={}", data_or(&res, "vip_type", "0")));
            res.cookie.push(format!("vip_token={}", data_or(&res, "vip_token", "")));
        }
        Ok(res)
    })
}

/// /login/wx/check —— 轮询微信扫码状态（直连 long.open.weixin.qq.com）
fn m_login_wx_check(query: Value, _use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        // Node 用模板串拼 URL（uuid 不做编码，原样复刻）
        let url = format!(
            "https://long.open.weixin.qq.com/connect/l/qrconnect?f=json&uuid={}",
            q_str(&query, "uuid")
        );
        match direct_request("GET", &url, &[]).await {
            Ok(body) => Ok(Response {
                status: 200,
                body,
                cookie: vec![],
                headers: Default::default(),
            }),
            Err(e) => Err(resp_502(Value::String(e))),
        }
    })
}

/// /login/wx/create —— 生成微信登录二维码（直连 api/open.weixin.qq.com）
fn m_login_wx_create(query: Value, _use_axios: UseAxios) -> ModuleFuture {
    Box::pin(async move {
        let _ = &query;
        let is_lite = config::is_lite();
        let (wx_appid, wx_secret) = if is_lite {
            (config::CONFIG.wx_lite_appid.clone(), config::CONFIG.wx_lite_secret.clone())
        } else {
            (config::CONFIG.wx_appid.clone(), config::CONFIG.wx_secret.clone())
        };

        // 1) access_token
        let at_resp = direct_request(
            "GET",
            "https://api.weixin.qq.com/cgi-bin/token",
            &[
                ("appid".into(), wx_appid.clone()),
                ("secret".into(), wx_secret),
                ("grant_type".into(), "client_credential".into()),
            ],
        )
        .await
        .map_err(|e| resp_502(Value::String(e)))?;
        let access_token = match at_resp.get("access_token").filter(|v| truthy(v)) {
            Some(v) => crate::jsval::js_to_string(v),
            None => return Err(resp_502(at_resp)),
        };

        // 2) sdk ticket
        let ticket_resp = direct_request(
            "GET",
            "https://api.weixin.qq.com/cgi-bin/ticket/getticket",
            &[("access_token".into(), access_token), ("type".into(), "2".into())],
        )
        .await
        .map_err(|e| resp_502(Value::String(e)))?;
        if ticket_resp.get("errcode").and_then(|v| v.as_i64()) != Some(0) {
            return Err(resp_502(ticket_resp));
        }
        let ticket = ticket_resp
            .get("ticket")
            .map(|v| crate::jsval::js_to_string(v))
            .unwrap_or_default();

        // 3) 签名 + qrconnect
        let timestamp = now_ms();
        let noncestr = crypto_md5(&random_string(16));
        let signature = crypto_sha1(&format!(
            "appid={}&noncestr={}&sdk_ticket={}&timestamp={}",
            wx_appid, noncestr, ticket, timestamp
        ));
        let mut connect = direct_request(
            "GET",
            "https://open.weixin.qq.com/connect/sdk/qrconnect",
            &[
                ("appid".into(), wx_appid),
                ("noncestr".into(), noncestr),
                ("timestamp".into(), timestamp.to_string()),
                ("scope".into(), "snsapi_userinfo".into()),
                ("signature".into(), signature),
            ],
        )
        .await
        .map_err(|e| resp_502(Value::String(e)))?;

        if connect.get("errcode").and_then(|v| v.as_i64()) != Some(0) {
            return Err(resp_502(connect));
        }
        // connect.data.qrcode['qrcodeurl'] = confirm 页链接
        let uuid = connect
            .get("uuid")
            .map(|v| crate::jsval::js_to_string(v))
            .unwrap_or_default();
        if let Some(qr) = connect.get_mut("qrcode").and_then(|v| v.as_object_mut()) {
            qr.insert(
                "qrcodeurl".into(),
                sv(format!("https://open.weixin.qq.com/connect/confirm?uuid={}", uuid)),
            );
        }
        Ok(Response {
            status: 200,
            body: connect,
            cookie: vec![],
            headers: Default::default(),
        })
    })
}

// ---------------- 工具 ----------------

fn str_map(pairs: &[(&str, &str)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), Value::String((*v).to_string()));
    }
    m
}

fn cookie_of(query: &Value) -> Map<String, Value> {
    match q_cookie(query) {
        Value::Object(m) => m.clone(),
        Value::String(s) => crate::util::cookie_to_json(s),
        _ => Map::new(),
    }
}

/// 生成二维码 PNG 的 base64（对应 qrcode.toDataURL）
fn generate_qr_base64(text: &str) -> Option<String> {
    let img = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let img = img.render::<image::Luma<u8>>().build();
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut buf);
        dyn_img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    }
    Some(format!("data:image/png;base64,{}", B64.encode(&buf)))
}

// ---------------- 类型别名 / 注册表 ----------------

pub type UseAxios =
    Arc<dyn Fn(RequestOptions) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Response>> + Send>> + Send + Sync>;
pub type ModuleFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Response>> + Send>>;

struct ModuleDef {
    route: &'static str,
    handler: ModuleFn,
}

/// route 注册表（Phase 1 样例；全量阶段将扩展到 155 个）
pub fn registry() -> Vec<ModuleDef> {
    vec![
        // —— Phase 1 样例 ——
        ModuleDef { route: "/search", handler: Arc::new(|q, u| m_search(q, u)) },
        ModuleDef { route: "/lyric", handler: Arc::new(|q, u| m_lyric(q, u)) },
        ModuleDef { route: "/album/detail", handler: Arc::new(|q, u| m_album_detail(q, u)) },
        ModuleDef { route: "/song/url", handler: Arc::new(|q, u| m_song_url(q, u)) },
        ModuleDef { route: "/server/now", handler: Arc::new(|q, u| m_server_now(q, u)) },
        ModuleDef { route: "/search/lyric", handler: Arc::new(|q, u| m_search_lyric(q, u)) },
        ModuleDef { route: "/login/qr/key", handler: Arc::new(|q, u| m_login_qr_key(q, u)) },
        ModuleDef { route: "/login/qr/create", handler: Arc::new(|q, u| m_login_qr_create(q, u)) },
        // —— 批次 1 ——
        ModuleDef { route: "/ai/recommend", handler: Arc::new(|q, u| m_ai_recommend(q, u)) },
        ModuleDef { route: "/album", handler: Arc::new(|q, u| m_album(q, u)) },
        ModuleDef { route: "/album/shop", handler: Arc::new(|q, u| m_album_shop(q, u)) },
        ModuleDef { route: "/album/songs", handler: Arc::new(|q, u| m_album_songs(q, u)) },
        ModuleDef { route: "/artist/albums", handler: Arc::new(|q, u| m_artist_albums(q, u)) },
        ModuleDef { route: "/artist/audios", handler: Arc::new(|q, u| m_artist_audios(q, u)) },
        ModuleDef { route: "/artist/detail", handler: Arc::new(|q, u| m_artist_detail(q, u)) },
        ModuleDef { route: "/artist/follow", handler: Arc::new(|q, u| m_artist_follow(q, u)) },
        ModuleDef { route: "/artist/follow/newsongs", handler: Arc::new(|q, u| m_artist_follow_newsongs(q, u)) },
        ModuleDef { route: "/artist/honour", handler: Arc::new(|q, u| m_artist_honour(q, u)) },
        ModuleDef { route: "/artist/lists", handler: Arc::new(|q, u| m_artist_lists(q, u)) },
        ModuleDef { route: "/artist/unfollow", handler: Arc::new(|q, u| m_artist_unfollow(q, u)) },
        ModuleDef { route: "/artist/videos", handler: Arc::new(|q, u| m_artist_videos(q, u)) },
        ModuleDef { route: "/audio", handler: Arc::new(|q, u| m_audio(q, u)) },
        ModuleDef { route: "/audio/accompany/matching", handler: Arc::new(|q, u| m_audio_accompany_matching(q, u)) },
        ModuleDef { route: "/audio/ktv/total", handler: Arc::new(|q, u| m_audio_ktv_total(q, u)) },
        ModuleDef { route: "/audio/related", handler: Arc::new(|q, u| m_audio_related(q, u)) },
        ModuleDef { route: "/brush", handler: Arc::new(|q, u| m_brush(q, u)) },
        ModuleDef { route: "/captcha/sent", handler: Arc::new(|q, u| m_captcha_sent(q, u)) },
        ModuleDef { route: "/comment/album", handler: Arc::new(|q, u| m_comment_album(q, u)) },
        ModuleDef { route: "/comment/count", handler: Arc::new(|q, u| m_comment_count(q, u)) },
        ModuleDef { route: "/comment/floor", handler: Arc::new(|q, u| m_comment_floor(q, u)) },
        ModuleDef { route: "/comment/music", handler: Arc::new(|q, u| m_comment_music(q, u)) },
        ModuleDef { route: "/comment/music/classify", handler: Arc::new(|q, u| m_comment_music_classify(q, u)) },
        ModuleDef { route: "/comment/music/hotword", handler: Arc::new(|q, u| m_comment_music_hotword(q, u)) },
        // —— 批次 2 ——
        ModuleDef { route: "/comment/playlist", handler: Arc::new(|q, u| m_comment_playlist(q, u)) },
        ModuleDef { route: "/everyday/friend", handler: Arc::new(|q, u| m_everyday_friend(q, u)) },
        ModuleDef { route: "/everyday/history", handler: Arc::new(|q, u| m_everyday_history(q, u)) },
        ModuleDef { route: "/everyday/recommend", handler: Arc::new(|q, u| m_everyday_recommend(q, u)) },
        ModuleDef { route: "/everyday/style/recommend", handler: Arc::new(|q, u| m_everyday_style_recommend(q, u)) },
        ModuleDef { route: "/favorite/count", handler: Arc::new(|q, u| m_favorite_count(q, u)) },
        ModuleDef { route: "/fm/class", handler: Arc::new(|q, u| m_fm_class(q, u)) },
        ModuleDef { route: "/fm/image", handler: Arc::new(|q, u| m_fm_image(q, u)) },
        ModuleDef { route: "/fm/recommend", handler: Arc::new(|q, u| m_fm_recommend(q, u)) },
        ModuleDef { route: "/fm/songs", handler: Arc::new(|q, u| m_fm_songs(q, u)) },
        ModuleDef { route: "/ip", handler: Arc::new(|q, u| m_ip(q, u)) },
        ModuleDef { route: "/ip/dateil", handler: Arc::new(|q, u| m_ip_dateil(q, u)) },
        ModuleDef { route: "/ip/playlist", handler: Arc::new(|q, u| m_ip_playlist(q, u)) },
        ModuleDef { route: "/ip/zone", handler: Arc::new(|q, u| m_ip_zone(q, u)) },
        ModuleDef { route: "/ip/zone/home", handler: Arc::new(|q, u| m_ip_zone_home(q, u)) },
        // —— 批次 3 ——
        ModuleDef { route: "/kmr/audio/mv", handler: Arc::new(|q, u| m_kmr_audio_mv(q, u)) },
        ModuleDef { route: "/krm/audio", handler: Arc::new(|q, u| m_krm_audio(q, u)) },
        ModuleDef { route: "/lastest/songs/listen", handler: Arc::new(|q, u| m_lastest_songs_listen(q, u)) },
        ModuleDef { route: "/longaudio/album/audios", handler: Arc::new(|q, u| m_longaudio_album_audios(q, u)) },
        ModuleDef { route: "/longaudio/album/detail", handler: Arc::new(|q, u| m_longaudio_album_detail(q, u)) },
        ModuleDef { route: "/longaudio/daily/recommend", handler: Arc::new(|q, u| m_longaudio_daily_recommend(q, u)) },
        ModuleDef { route: "/longaudio/rank/recommend", handler: Arc::new(|q, u| m_longaudio_rank_recommend(q, u)) },
        ModuleDef { route: "/longaudio/vip/recommend", handler: Arc::new(|q, u| m_longaudio_vip_recommend(q, u)) },
        ModuleDef { route: "/longaudio/week/recommend", handler: Arc::new(|q, u| m_longaudio_week_recommend(q, u)) },
        // —— 批次 4（login 系列） ——
        ModuleDef { route: "/login", handler: Arc::new(|q, u| m_login(q, u)) },
        ModuleDef { route: "/login/cellphone", handler: Arc::new(|q, u| m_login_cellphone(q, u)) },
        ModuleDef { route: "/login/token", handler: Arc::new(|q, u| m_login_token(q, u)) },
        ModuleDef { route: "/login/qr/check", handler: Arc::new(|q, u| m_login_qr_check(q, u)) },
        ModuleDef { route: "/login/device", handler: Arc::new(|q, u| m_login_device(q, u)) },
        ModuleDef { route: "/login/device/kick", handler: Arc::new(|q, u| m_login_device_kick(q, u)) },
        // —— 批次 5（images / search / recommend） ——
        ModuleDef { route: "/images", handler: Arc::new(|q, u| m_images(q, u)) },
        ModuleDef { route: "/images/audio", handler: Arc::new(|q, u| m_images_audio(q, u)) },
        ModuleDef { route: "/search/complex", handler: Arc::new(|q, u| m_search_complex(q, u)) },
        ModuleDef { route: "/search/default", handler: Arc::new(|q, u| m_search_default(q, u)) },
        ModuleDef { route: "/search/hot", handler: Arc::new(|q, u| m_search_hot(q, u)) },
        ModuleDef { route: "/search/mixed", handler: Arc::new(|q, u| m_search_mixed(q, u)) },
        ModuleDef { route: "/search/suggest", handler: Arc::new(|q, u| m_search_suggest(q, u)) },
        ModuleDef { route: "/recommend/songs", handler: Arc::new(|q, u| m_recommend_songs(q, u)) },
        // —— 批次 6（rank / singer / song） ——
        ModuleDef { route: "/rank/audio", handler: Arc::new(|q, u| m_rank_audio(q, u)) },
        ModuleDef { route: "/rank/info", handler: Arc::new(|q, u| m_rank_info(q, u)) },
        ModuleDef { route: "/rank/list", handler: Arc::new(|q, u| m_rank_list(q, u)) },
        ModuleDef { route: "/rank/top", handler: Arc::new(|q, u| m_rank_top(q, u)) },
        ModuleDef { route: "/rank/vol", handler: Arc::new(|q, u| m_rank_vol(q, u)) },
        ModuleDef { route: "/singer/list", handler: Arc::new(|q, u| m_singer_list(q, u)) },
        ModuleDef { route: "/song/climax", handler: Arc::new(|q, u| m_song_climax(q, u)) },
        ModuleDef { route: "/song/ranking", handler: Arc::new(|q, u| m_song_ranking(q, u)) },
        ModuleDef { route: "/song/ranking/filter", handler: Arc::new(|q, u| m_song_ranking_filter(q, u)) },
        ModuleDef { route: "/song/url/new", handler: Arc::new(|q, u| m_song_url_new(q, u)) },
        // —— 批次 7（sheet / top） ——
        ModuleDef { route: "/sheet/collection", handler: Arc::new(|q, u| m_sheet_collection(q, u)) },
        ModuleDef { route: "/sheet/collection/detail", handler: Arc::new(|q, u| m_sheet_collection_detail(q, u)) },
        ModuleDef { route: "/sheet/detail", handler: Arc::new(|q, u| m_sheet_detail(q, u)) },
        ModuleDef { route: "/sheet/hot", handler: Arc::new(|q, u| m_sheet_hot(q, u)) },
        ModuleDef { route: "/sheet/list", handler: Arc::new(|q, u| m_sheet_list(q, u)) },
        ModuleDef { route: "/top/album", handler: Arc::new(|q, u| m_top_album(q, u)) },
        ModuleDef { route: "/top/card", handler: Arc::new(|q, u| m_top_card(q, u)) },
        ModuleDef { route: "/top/card/youth", handler: Arc::new(|q, u| m_top_card_youth(q, u)) },
        ModuleDef { route: "/top/ip", handler: Arc::new(|q, u| m_top_ip(q, u)) },
        ModuleDef { route: "/top/playlist", handler: Arc::new(|q, u| m_top_playlist(q, u)) },
        ModuleDef { route: "/top/song", handler: Arc::new(|q, u| m_top_song(q, u)) },
        // —— 批次 8（scene / yueku / theme） ——
        ModuleDef { route: "/scene/audio/list", handler: Arc::new(|q, u| m_scene_audio_list(q, u)) },
        ModuleDef { route: "/scene/collection/list", handler: Arc::new(|q, u| m_scene_collection_list(q, u)) },
        ModuleDef { route: "/scene/lists", handler: Arc::new(|q, u| m_scene_lists(q, u)) },
        ModuleDef { route: "/scene/lists/v2", handler: Arc::new(|q, u| m_scene_lists_v2(q, u)) },
        ModuleDef { route: "/scene/module", handler: Arc::new(|q, u| m_scene_module(q, u)) },
        ModuleDef { route: "/scene/module/info", handler: Arc::new(|q, u| m_scene_module_info(q, u)) },
        ModuleDef { route: "/scene/music", handler: Arc::new(|q, u| m_scene_music(q, u)) },
        ModuleDef { route: "/scene/video/list", handler: Arc::new(|q, u| m_scene_video_list(q, u)) },
        ModuleDef { route: "/yueku", handler: Arc::new(|q, u| m_yueku(q, u)) },
        ModuleDef { route: "/yueku/banner", handler: Arc::new(|q, u| m_yueku_banner(q, u)) },
        ModuleDef { route: "/yueku/fm", handler: Arc::new(|q, u| m_yueku_fm(q, u)) },
        ModuleDef { route: "/theme/music", handler: Arc::new(|q, u| m_theme_music(q, u)) },
        ModuleDef { route: "/theme/music/detail", handler: Arc::new(|q, u| m_theme_music_detail(q, u)) },
        ModuleDef { route: "/theme/playlist", handler: Arc::new(|q, u| m_theme_playlist(q, u)) },
        ModuleDef { route: "/theme/playlist/track", handler: Arc::new(|q, u| m_theme_playlist_track(q, u)) },
        // —— 批次 9（youth 系列） ——
        ModuleDef { route: "/youth/channel/all", handler: Arc::new(|q, u| m_youth_channel_all(q, u)) },
        ModuleDef { route: "/youth/channel/amway", handler: Arc::new(|q, u| m_youth_channel_amway(q, u)) },
        ModuleDef { route: "/youth/channel/detail", handler: Arc::new(|q, u| m_youth_channel_detail(q, u)) },
        ModuleDef { route: "/youth/channel/similar", handler: Arc::new(|q, u| m_youth_channel_similar(q, u)) },
        ModuleDef { route: "/youth/channel/song", handler: Arc::new(|q, u| m_youth_channel_song(q, u)) },
        ModuleDef { route: "/youth/channel/song/detail", handler: Arc::new(|q, u| m_youth_channel_song_detail(q, u)) },
        ModuleDef { route: "/youth/channel/sub", handler: Arc::new(|q, u| m_youth_channel_sub(q, u)) },
        ModuleDef { route: "/youth/day/vip", handler: Arc::new(|q, u| m_youth_day_vip(q, u)) },
        ModuleDef { route: "/youth/day/vip/upgrade", handler: Arc::new(|q, u| m_youth_day_vip_upgrade(q, u)) },
        ModuleDef { route: "/youth/dynamic", handler: Arc::new(|q, u| m_youth_dynamic(q, u)) },
        ModuleDef { route: "/youth/dynamic/recent", handler: Arc::new(|q, u| m_youth_dynamic_recent(q, u)) },
        ModuleDef { route: "/youth/listen/song", handler: Arc::new(|q, u| m_youth_listen_song(q, u)) },
        ModuleDef { route: "/youth/month/vip/record", handler: Arc::new(|q, u| m_youth_month_vip_record(q, u)) },
        ModuleDef { route: "/youth/union/vip", handler: Arc::new(|q, u| m_youth_union_vip(q, u)) },
        ModuleDef { route: "/youth/user/song", handler: Arc::new(|q, u| m_youth_user_song(q, u)) },
        ModuleDef { route: "/youth/vip", handler: Arc::new(|q, u| m_youth_vip(q, u)) },
        // —— 批次 10（playlist 系列） ——
        ModuleDef { route: "/playlist/add", handler: Arc::new(|q, u| m_playlist_add(q, u)) },
        ModuleDef { route: "/playlist/del", handler: Arc::new(|q, u| m_playlist_del(q, u)) },
        ModuleDef { route: "/playlist/detail", handler: Arc::new(|q, u| m_playlist_detail(q, u)) },
        ModuleDef { route: "/playlist/effect", handler: Arc::new(|q, u| m_playlist_effect(q, u)) },
        ModuleDef { route: "/playlist/similar", handler: Arc::new(|q, u| m_playlist_similar(q, u)) },
        ModuleDef { route: "/playlist/tags", handler: Arc::new(|q, u| m_playlist_tags(q, u)) },
        ModuleDef { route: "/playlist/track/all", handler: Arc::new(|q, u| m_playlist_track_all(q, u)) },
        ModuleDef { route: "/playlist/track/all/new", handler: Arc::new(|q, u| m_playlist_track_all_new(q, u)) },
        ModuleDef { route: "/playlist/tracks/add", handler: Arc::new(|q, u| m_playlist_tracks_add(q, u)) },
        ModuleDef { route: "/playlist/tracks/del", handler: Arc::new(|q, u| m_playlist_tracks_del(q, u)) },
        // —— 批次 11（user / video / 微信直连 / 收尾，至此 155/155） ——
        ModuleDef { route: "/login/openplat", handler: Arc::new(|q, u| m_login_openplat(q, u)) },
        ModuleDef { route: "/login/wx/check", handler: Arc::new(|q, u| m_login_wx_check(q, u)) },
        ModuleDef { route: "/login/wx/create", handler: Arc::new(|q, u| m_login_wx_create(q, u)) },
        ModuleDef { route: "/pc/diantai", handler: Arc::new(|q, u| m_pc_diantai(q, u)) },
        ModuleDef { route: "/personal/fm", handler: Arc::new(|q, u| m_personal_fm(q, u)) },
        ModuleDef { route: "/playhistory/upload", handler: Arc::new(|q, u| m_playhistory_upload(q, u)) },
        ModuleDef { route: "/privilege/lite", handler: Arc::new(|q, u| m_privilege_lite(q, u)) },
        ModuleDef { route: "/register/dev", handler: Arc::new(|q, u| m_register_dev(q, u)) },
        ModuleDef { route: "/user/cloud", handler: Arc::new(|q, u| m_user_cloud(q, u)) },
        ModuleDef { route: "/user/cloud/url", handler: Arc::new(|q, u| m_user_cloud_url(q, u)) },
        ModuleDef { route: "/user/detail", handler: Arc::new(|q, u| m_user_detail(q, u)) },
        ModuleDef { route: "/user/follow", handler: Arc::new(|q, u| m_user_follow(q, u)) },
        ModuleDef { route: "/user/follow/message", handler: Arc::new(|q, u| m_user_follow_message(q, u)) },
        ModuleDef { route: "/user/history", handler: Arc::new(|q, u| m_user_history(q, u)) },
        ModuleDef { route: "/user/listen", handler: Arc::new(|q, u| m_user_listen(q, u)) },
        ModuleDef { route: "/user/playlist", handler: Arc::new(|q, u| m_user_playlist(q, u)) },
        ModuleDef { route: "/user/video/collect", handler: Arc::new(|q, u| m_user_video_collect(q, u)) },
        ModuleDef { route: "/user/video/love", handler: Arc::new(|q, u| m_user_video_love(q, u)) },
        ModuleDef { route: "/user/vip/detail", handler: Arc::new(|q, u| m_user_vip_detail(q, u)) },
        ModuleDef { route: "/video/detail", handler: Arc::new(|q, u| m_video_detail(q, u)) },
        ModuleDef { route: "/video/privilege", handler: Arc::new(|q, u| m_video_privilege(q, u)) },
        ModuleDef { route: "/video/url", handler: Arc::new(|q, u| m_video_url(q, u)) },
    ]
}

/// 按 route 查找模块处理函数
pub fn lookup(route: &str) -> Option<ModuleFn> {
    registry().into_iter().find(|d| d.route == route).map(|d| d.handler)
}

#[allow(dead_code)]
fn _unused(_q: Value) {
    let _ = q_clone(&_q);
    let _ = ResponseType::ArrayBuffer;
    let _ = calculate_mid("");
}
