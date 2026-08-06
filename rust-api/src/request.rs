//! 对应 util/request.js —— createRequest 请求构造与发送。
//!
//! 行为严格对齐 Node 版，包括所有「怪异」细节：
//! - headers 里 dfid/clienttime/mid 出现两遍（Object.assign 合并），最终取 params.clienttime 值，二者一致。
//! - cookie 永不作为 Cookie 头发送，认证走 query + headers。
//! - `notSignature`（如 song_url 的 notSign 拼写错误）**仍会**计算 signature。
//! - encryptType: android(默认) / web / register；register 为死代码但保留。
//! - openapicdn baseURL：把 params 内联成 query string（**不编码**），params 清空。
//! - 响应：status===0 或 error_code!==0 -> 502 reject；否则 200 resolve。
//! - 始终返回 Response（含 status），由上层决定 resolve/reject 语义（见 handler）。

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::config::{use_appid, use_clientver};
use crate::jsval::js_to_string;
use crate::sign::{self, EncryptType};
use crate::util::parse_cookie_string;

/// 响应类型（对应 responseType: 'arraybuffer'）
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseType {
    Json,
    ArrayBuffer,
}

impl Default for ResponseType {
    fn default() -> Self {
        ResponseType::Json
    }
}

/// createRequest 入参（对应 options）
#[derive(Debug, Clone)]
pub struct RequestOptions {
    pub method: String,
    pub url: String,
    pub base_url: Option<String>,
    pub params: Map<String, Value>,
    pub data: Option<Value>,
    /// 原始二进制 body（对应 Node `data: Buffer.from(base64,'base64')`）。
    /// 若设置，则直接以原始字节作为 POST body 发送，优先级高于 `data`。
    /// 注意：使用 data_binary 的模块（user_cloud）均为 not_signature=true，
    /// 因此该二进制不参与签名计算。
    pub data_binary: Option<Vec<u8>>,
    /// 二进制签名负载（对应酷狗 Android 流式 MD5：`secret + paramsString + bytes + secret`）。
    /// 仅当 encrypt_type == Android 且本字段为 Some 时生效，覆盖普通 signature_android。
    /// 听歌识曲（audio_match）等需要把原始音频字节纳入签名的场景使用。
    pub sign_binary: Option<Vec<u8>>,
    pub headers: Map<String, Value>,
    pub encrypt_type: EncryptType,
    pub cookie: Map<String, Value>,
    pub encrypt_key: bool,
    pub clear_default_params: bool,
    pub not_signature: bool,
    pub ip: Option<String>,
    pub real_ip: Option<String>,
    pub response_type: ResponseType,
}

impl Default for RequestOptions {
    fn default() -> Self {
        RequestOptions {
            method: "get".to_string(),
            url: String::new(),
            base_url: None,
            params: Map::new(),
            data: None,
            data_binary: None,
            sign_binary: None,
            headers: Map::new(),
            encrypt_type: EncryptType::Android,
            cookie: Map::new(),
            encrypt_key: false,
            clear_default_params: false,
            not_signature: false,
            ip: None,
            real_ip: None,
            response_type: ResponseType::Json,
        }
    }
}

/// 响应（对应 UseAxiosResponse: {status, body, cookie, headers}）
/// body 用 serde_json::Value 统一承载（数组/对象/字符串皆可；arraybuffer 场景为字符串）
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Value,
    pub cookie: Vec<String>,
    pub headers: HashMap<String, String>,
}

fn get_str<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    m.get(key).and_then(|v| v.as_str())
}

/// 把 params (Map) 转为 (key, value 字符串) 列表，值按 JS `${}` 规则强制转换，跳过 null。
/// 保留插入顺序（preserve_order）。
fn params_to_pairs(params: &Map<String, Value>) -> Vec<(String, String)> {
    let mut out = Vec::with_capacity(params.len());
    for (k, v) in params {
        if v.is_null() {
            // axios 会跳过 null 值参数
            continue;
        }
        out.push((k.clone(), js_to_string(v)));
    }
    out
}

/// 根据 KUGOU_API_PROXY 环境变量构造 reqwest 代理（对应 runtime.resolveProxy）。
/// 支持 http(s)://[user:pass@]host:port
fn build_proxy() -> Option<reqwest::Proxy> {
    let raw = std::env::var("KUGOU_API_PROXY").ok()?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    // 简单解析：协议/鉴权/主机/端口
    let body = if raw.contains("://") {
        raw.clone()
    } else {
        format!("http://{}", raw)
    };
    let proxy = reqwest::Proxy::all(&body).ok()?;
    Some(proxy)
}

fn build_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent("Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi")
        .gzip(true);
    if let Some(p) = build_proxy() {
        builder = builder.proxy(p);
    }
    builder
        .build()
        .expect("reqwest client 初始化失败")
}

/// 对应 createRequest(options) -> {status, body, cookie, headers}
pub async fn create_request(options: RequestOptions) -> Result<Response, Response> {
    // —— 默认头/参数所需的基础量 ——
    let dfid = get_str(&options.cookie, "dfid").unwrap_or("-").to_string();
    // mid 由 KUGOU_API_MID 强制字符串化（对应 Node `${options?.cookie?.KUGOU_API_MID}`）。
    // 键缺失时得到字符串 "undefined"（而非 "-"）—— 影响 captcha_sent 等传自定义 cookie 的模块。
    let mid = options
        .cookie
        .get("KUGOU_API_MID")
        .map(|v| js_to_string(v))
        .unwrap_or_else(|| "undefined".to_string());
    let uuid = "-".to_string();
    let token = get_str(&options.cookie, "token").unwrap_or("").to_string();
    let userid = options.cookie.get("userid").cloned().unwrap_or(Value::Null);
    let clienttime = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()) as i64;
    let ip = options.real_ip.clone().or_else(|| options.ip.clone()).unwrap_or_default();

    // —— 默认参数（保留顺序）——
    let mut default_params: Map<String, Value> = Map::new();
    default_params.insert("dfid".into(), Value::String(dfid.clone()));
    default_params.insert("mid".into(), Value::String(mid.clone()));
    default_params.insert("uuid".into(), Value::String(uuid.clone()));
    default_params.insert("appid".into(), Value::from(use_appid()));
    default_params.insert("clientver".into(), Value::from(use_clientver()));
    default_params.insert("clienttime".into(), Value::from(clienttime));
    if !token.is_empty() {
        default_params.insert("token".into(), Value::String(token.clone()));
    }
    // Node: if (userid && userid !== 0)
    let userid_zero = matches!(userid, Value::Null)
        || matches!(userid, Value::Number(ref n) if n.as_i64() == Some(0));
    if !userid_zero {
        default_params.insert("userid".into(), userid.clone());
    }

    // params = clearDefaultParams ? params : {...default, ...params}
    let mut params = if options.clear_default_params {
        options.params.clone()
    } else {
        let mut merged = default_params.clone();
        for (k, v) in &options.params {
            merged.insert(k.clone(), v.clone());
        }
        merged
    };

    // headers 基础集合
    let mut headers: Map<String, Value> = Map::new();
    headers.insert("dfid".into(), Value::String(dfid.clone()));
    headers.insert("clienttime".into(), Value::from(clienttime));
    headers.insert("mid".into(), Value::String(mid.clone()));
    headers.insert("kg-rc".into(), Value::from(1));
    headers.insert("kg-thash".into(), Value::String("5d816a0".into()));
    headers.insert("kg-rec".into(), Value::from(1));
    headers.insert(
        "kg-rf".into(),
        Value::String("B9EDA08A64250DEFFBCADDEE00F8F25F".into()),
    );
    if !ip.is_empty() {
        headers.insert("X-Real-IP".into(), Value::String(ip.clone()));
        headers.insert("X-Forwarded-For".into(), Value::String(ip.clone()));
    }

    // encryptKey：注入 params.key = signKey(hash, mid, userid, appid)
    if options.encrypt_key {
        let hash = params
            .get("hash")
            .map(|v| js_to_string(v))
            .unwrap_or_default();
        let p_mid = params.get("mid").map(|v| js_to_string(v)).unwrap_or_default();
        let p_userid = params
            .get("userid")
            .map(|v| js_to_string(v))
            .unwrap_or_default();
        let p_appid = params
            .get("appid")
            .map(|v| js_to_string(v))
            .unwrap_or_else(|| use_appid().to_string());
        let key = sign::sign_key(&hash, &p_mid, &p_userid, &p_appid);
        params.insert("key".into(), Value::String(key));
    }

    // data 字符串化（对象 -> JSON.stringify）
    let data_str = match &options.data {
        Some(Value::Object(_)) | Some(Value::Array(_)) => {
            serde_json::to_string(options.data.as_ref().unwrap()).unwrap_or_default()
        }
        Some(Value::String(s)) => s.clone(),
        Some(other) => js_to_string(other),
        None => String::new(),
    };

    // signature（notSignature 时跳过；params 已有 signature 也跳过）
    if params.get("signature").is_none() && !options.not_signature {
        match options.encrypt_type {
            EncryptType::Register => {
                params.insert(
                    "signature".into(),
                    Value::String(sign::signature_register(&params)),
                );
            }
            EncryptType::Web => {
                params.insert(
                    "signature".into(),
                    Value::String(sign::signature_web(&params)),
                );
            }
            EncryptType::Android => {
                let sig = if let Some(bin) = &options.sign_binary {
                    sign::signature_android_binary(&params, bin)
                } else {
                    sign::signature_android(&params, &data_str)
                };
                params.insert("signature".into(), Value::String(sig));
            }
        }
    }

    // headers 最终合并：默认 UA + options.headers + dfid/clienttime/mid
    let mut final_headers: Map<String, Value> = Map::new();
    final_headers.insert(
        "User-Agent".into(),
        Value::String("Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi".into()),
    );
    for (k, v) in &options.headers {
        final_headers.insert(k.clone(), v.clone());
    }
    for (k, v) in &headers {
        final_headers.insert(k.clone(), v.clone());
    }

    // baseURL 默认
    let base_url = options
        .base_url
        .clone()
        .unwrap_or_else(|| "https://gateway.kugou.com".to_string());

    // openapicdn：把 params 内联进 url，params 清空（不编码，与 Node 一致）
    let (final_url, final_params) = if base_url.contains("openapicdn") {
        let query = params_to_pairs(&params)
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let sep = if options.url.contains('?') { '&' } else { '?' };
        let u = format!("{}{}{}", options.url, sep, query);
        (u, Map::new())
    } else {
        (options.url.clone(), params.clone())
    };

    // —— 发送 ——
    let client = build_client();
    let method = match options.method.to_uppercase().as_str() {
        "POST" => reqwest::Method::POST,
        _ => reqwest::Method::GET,
    };
    let mut req_builder = client
        .request(method.clone(), &format!("{}{}", base_url, final_url));

    // 头部（内容类型默认 json，若 data 存在）
    for (k, v) in &final_headers {
        if let Some(s) = v.as_str() {
            req_builder = req_builder.header(k, s);
        } else if let Some(n) = v.as_i64() {
            req_builder = req_builder.header(k, n.to_string());
        }
    }

    // query
    if !final_params.is_empty() {
        req_builder = req_builder.query(&params_to_pairs(&final_params));
    }

    // body：优先原始二进制（对应 Buffer.from(base64,'base64')）；否则 data 字符串化后发送。
    // Content-Type 复刻 axios transformRequest 语义：
    //   - 对象/数组 payload -> application/json（axios setContentType('application/json')）
    //   - 字符串 / Buffer  -> 不设置 Content-Type（axios 对字符串与 Buffer 原样透传）
    // 若模块 headers 已显式给出 Content-Type（如 privilege_lite），上方 final_headers 已设置，此处不覆盖。
    let has_explicit_ct = final_headers
        .keys()
        .any(|k| k.eq_ignore_ascii_case("content-type"));
    if let Some(bin) = &options.data_binary {
        req_builder = req_builder.body(bin.clone());
    } else if options.data.is_some() {
        let is_obj_payload = matches!(
            options.data,
            Some(Value::Object(_)) | Some(Value::Array(_))
        );
        if is_obj_payload && !has_explicit_ct {
            req_builder = req_builder.header("Content-Type", "application/json");
        }
        req_builder = req_builder.body(data_str.clone());
    }

    let mut answer = Response {
        status: 500,
        body: Value::Null,
        cookie: Vec::new(),
        headers: HashMap::new(),
    };

    match req_builder.send().await {
        Ok(response) => {
            // set-cookie -> 解析为纯 cookie 串
            let set_cookies: Vec<String> = response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok().map(|s| parse_cookie_string(s)))
                .collect();
            answer.cookie = set_cookies;
            if let Some(ssa) = response.headers().get("ssa-code").and_then(|v| v.to_str().ok()) {
                answer.headers.insert("ssa-code".into(), ssa.to_string());
            }

            let body_bytes = response.bytes().await.unwrap_or_default();

            // arraybuffer：对应 Node responseType:'arraybuffer' —— body 为原始 Buffer。
            // Node 里 `response.data.status`/`error_code` 取自 Buffer（恒 undefined），
            // 故错误判定永不触发，始终 resolve(200)。模块侧用 `body.toString('base64')`
            // 取出 base64 再解密，这里直接把原始字节 base64 编码存入 body 字符串。
            if options.response_type == ResponseType::ArrayBuffer {
                answer.status = 200;
                answer.body = Value::String(B64.encode(&body_bytes));
                return Ok(answer);
            }

            let parsed: Value = match serde_json::from_slice::<Value>(&body_bytes) {
                Ok(v) => v,
                Err(_) => Value::String(String::from_utf8_lossy(&body_bytes).to_string()),
            };

            let is_err = parsed.get("status").and_then(|v| v.as_i64()) == Some(0)
                || parsed
                    .get("error_code")
                    .and_then(|v| v.as_i64())
                    .map(|c| c != 0)
                    .unwrap_or(false);

            if is_err {
                answer.status = 502;
                answer.body = parsed;
                Err(answer)
            } else {
                answer.status = 200;
                answer.body = parsed;
                Ok(answer)
            }
        }
        Err(e) => {
            answer.status = 502;
            answer.body = serde_json::json!({ "status": 0, "msg": e.to_string() });
            Err(answer)
        }
    }
}
