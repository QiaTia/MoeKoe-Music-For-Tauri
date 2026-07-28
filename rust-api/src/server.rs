//! 对应 server.js —— axum 服务、中间件链、统一路由分发。
//!
//! 复刻要点：
//! - CORS + OPTIONS 预检（非根路径且不含 '.' 时下发 CORS 头）
//! - Cookie 头解析
//! - 平台 Cookie 注入（KUGOU_API_PLATFORM/MID/GUID/DEV/MAC）并 Set-Cookie
//! - body 解析（json / urlencoded）
//! - query 合并：{ cookie:{...req.cookies, ...queryCookie, ...auth}, ...params, body }
//! - 模块分发；命中则写回 Set-Cookie / ssa-code / status / body，否则 404 信封

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::Response as AxumResponse,
    Router,
};
use serde_json::{Map, Value};

use crate::crypto::crypto_md5;
use crate::modules::{self, UseAxios};
use crate::request::{RequestOptions, Response};
use crate::util::{calculate_mid, cookie_to_json, get_guid, random_string};

/// 进程级共享状态（对应 server.js 顶部的 guid / serverDev）
pub struct AppState {
    pub guid: String,
    pub server_dev: String,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            guid: crypto_md5(&get_guid()),
            server_dev: random_string(10).to_uppercase(),
        }
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new().fallback(handler).with_state(state)
}

// ---------------- 响应缓存（对应 util/apicache.js，'2 minutes'，仅缓存 200） ----------------

const CACHE_DURATION_SECS: u64 = 120; // '2 minutes'

/// 缓存条目：状态码 + 全部响应头（含 set-cookie/cache-control）+ body 字节 + 存入时刻
struct CachedEntry {
    status: StatusCode,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Bytes,
    stored_at: std::time::Instant,
}

static RESPONSE_CACHE: once_cell::sync::Lazy<moka::future::Cache<String, Arc<CachedEntry>>> =
    once_cell::sync::Lazy::new(|| {
        moka::future::Cache::builder()
            .time_to_live(std::time::Duration::from_secs(CACHE_DURATION_SECS))
            .build()
    });

/// Express req.hostname：Host 头去端口（IPv6 保留方括号内内容）
fn hostname_of(headers: &HeaderMap) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if let Some(rest) = host.strip_prefix('[') {
        // [::1]:3000 -> ::1（Express 对 IPv6 返回去括号主机名）
        return rest.split(']').next().unwrap_or("").to_string();
    }
    host.split(':').next().unwrap_or("").to_string()
}

/// 统一入口：apicache 语义的缓存包装（bypass 头跳过；命中重放；仅缓存 200）
async fn handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> AxumResponse {
    // OPTIONS 在 Express 里于 CORS 中间件短路，先于 cache —— 不参与缓存
    if method == Method::OPTIONS {
        return handle_request(state, method, uri, headers, body).await;
    }
    // x-apicache-bypass / x-apicache-force-fetch -> 直接放行（且不加 cache-control）
    if headers.contains_key("x-apicache-bypass") || headers.contains_key("x-apicache-force-fetch") {
        return handle_request(state, method, uri, headers, body).await;
    }

    // key = req.hostname + req.originalUrl（path + query）
    let key = format!(
        "{}{}",
        hostname_of(&headers),
        uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(uri.path())
    );

    // —— 命中：重放缓存（cache-control 按剩余时间递减，对应 sendCachedResponse） ——
    if let Some(entry) = RESPONSE_CACHE.get(&key).await {
        let mut resp = AxumResponse::new(axum::body::Body::from(entry.body.clone()));
        *resp.status_mut() = entry.status;
        for (k, v) in &entry.headers {
            resp.headers_mut().append(k.clone(), v.clone());
        }
        let remaining = (CACHE_DURATION_SECS as f64 - entry.stored_at.elapsed().as_secs_f64())
            .round()
            .max(0.0) as u64;
        if let Ok(val) = HeaderValue::from_str(&format!("max-age={}", remaining)) {
            resp.headers_mut()
                .insert(axum::http::header::CACHE_CONTROL, val);
        }
        return resp;
    }

    // —— 未命中：执行业务，再按状态码决定缓存与 cache-control ——
    let resp = handle_request(state, method, uri, headers, body).await;
    let (mut parts, resp_body) = resp.into_parts();
    let body_bytes = axum::body::to_bytes(resp_body, usize::MAX)
        .await
        .unwrap_or_default();

    if parts.status == StatusCode::OK && !body_bytes.is_empty() {
        // 复刻 apicache：缓存前先写入 cache-control: max-age=120，随响应头一并存储
        if let Ok(val) = HeaderValue::from_str(&format!("max-age={}", CACHE_DURATION_SECS)) {
            parts.headers.insert(axum::http::header::CACHE_CONTROL, val);
        }
        let stored_headers: Vec<(HeaderName, HeaderValue)> = parts
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        RESPONSE_CACHE
            .insert(
                key,
                Arc::new(CachedEntry {
                    status: parts.status,
                    headers: stored_headers,
                    body: body_bytes.clone(),
                    stored_at: std::time::Instant::now(),
                }),
            )
            .await;
    } else {
        // 非 200：no-cache, no-store, must-revalidate
        parts.headers.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        );
    }

    AxumResponse::from_parts(parts, axum::body::Body::from(body_bytes))
}

/// 统一分发（原 handler；对应 server.js 的中间件链 + 路由分发）
async fn handle_request(
    state: Arc<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> AxumResponse {
    let path = uri.path().to_string();

    // —— CORS / OPTIONS ——
    let is_cors_path = path != "/" && !path.contains('.');
    let origin = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();
    let cors_origin = std::env::var("CORS_ALLOW_ORIGIN").unwrap_or_else(|_| origin.clone());

    if method == Method::OPTIONS {
        let mut resp = AxumResponse::new(axum::body::Body::empty());
        *resp.status_mut() = StatusCode::NO_CONTENT;
        if is_cors_path {
            apply_cors(resp.headers_mut(), &cors_origin);
        }
        return resp;
    }

    // —— Cookie 解析 ——
    let mut cookies = parse_cookie_header(&headers);

    // —— 平台 Cookie 注入 + Set-Cookie ——
    let is_https = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("https"))
        .unwrap_or(false);
    let cookie_suffix = if is_https {
        "; PATH=/; SameSite=None; Secure"
    } else {
        "; PATH=/"
    };
    let mut set_cookie_headers: Vec<String> = Vec::new();

    let platform = std::env::var("platform").unwrap_or_default();
    let guid_env = std::env::var("KUGOU_API_GUID").unwrap_or_else(|_| state.guid.clone());
    let mid = calculate_mid(&guid_env);
    let dev = std::env::var("KUGOU_API_DEV")
        .unwrap_or_else(|_| state.server_dev.clone())
        .to_uppercase();
    let mac = std::env::var("KUGOU_API_MAC")
        .unwrap_or_else(|_| "02:00:00:00:00:00".to_string())
        .to_uppercase();

    let mut ensure = |key: &str, value: String| {
        if !cookies.contains_key(key) {
            cookies.insert(key.to_string(), value.clone());
            set_cookie_headers.push(format!("{}={}{}", key, value, cookie_suffix));
        }
    };
    ensure("KUGOU_API_PLATFORM", platform);
    ensure("KUGOU_API_MID", mid);
    ensure("KUGOU_API_GUID", guid_env);
    ensure("KUGOU_API_DEV", dev);
    ensure("KUGOU_API_MAC", mac);

    // —— 查询参数解析 ——
    let (mut params, query_cookie) = parse_query(uri.query().unwrap_or(""));

    // —— body 解析 ——
    let body_value = parse_body(&headers, &body);

    // —— 组合 cookie 对象：{...req.cookies, ...queryCookie, ...auth} ——
    let mut cookie_obj: Map<String, Value> = Map::new();
    for (k, v) in &cookies {
        cookie_obj.insert(k.clone(), Value::String(v.clone()));
    }
    if let Some(qc) = query_cookie {
        for (k, v) in qc {
            cookie_obj.insert(k, v);
        }
    }
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        for (k, v) in cookie_to_json(auth) {
            cookie_obj.insert(k, v);
        }
    }

    // —— query = { cookie, ...params, body } ——
    let mut query = Map::new();
    query.insert("cookie".into(), Value::Object(cookie_obj));
    for (k, v) in std::mem::take(&mut params) {
        query.insert(k, v);
    }
    query.insert("body".into(), body_value);
    let query_value = Value::Object(query.clone());

    // no_cookie 标记
    let no_cookie = query
        .get("noCookie")
        .map(|v| !matches!(v, Value::Null) && v.as_str() != Some("0") && v.as_bool() != Some(false))
        .unwrap_or(false);

    // —— 模块分发 ——
    let module_fn = match modules::lookup(&path) {
        Some(f) => f,
        None => {
            // 404 信封
            let mut resp = json_response(
                StatusCode::NOT_FOUND,
                &serde_json::json!({ "code": 404, "data": null, "msg": "Not Found" }),
            );
            if is_cors_path {
                apply_cors(resp.headers_mut(), &cors_origin);
            }
            append_set_cookies(resp.headers_mut(), &set_cookie_headers);
            return resp;
        }
    };

    // 客户端 IP（近似 req.ip）
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let ip_for_axios = ip.clone();
    let use_axios: UseAxios = Arc::new(move |mut opts: RequestOptions| {
        opts.ip = Some(ip_for_axios.clone());
        Box::pin(crate::request::create_request(opts))
    });

    let module_result = module_fn(query_value, use_axios).await;

    // 复刻 server.js catch 分支：模块 reject(抛错) 且 error.body 为空(null/undefined)
    // -> 返回 404 信封（如 login_device_kick 的未定义变量 ReferenceError）。
    // 仅作用于 Err 路径；Ok 路径即使 body 为 null 也原样回送。
    if let Err(ref e) = module_result {
        if e.body.is_null() {
            let mut resp = json_response(
                StatusCode::NOT_FOUND,
                &serde_json::json!({ "code": 404, "data": null, "msg": "Not Found" }),
            );
            if is_cors_path {
                apply_cors(resp.headers_mut(), &cors_origin);
            }
            append_set_cookies(resp.headers_mut(), &set_cookie_headers);
            return resp;
        }
    }

    let (module_resp, _is_err): (Response, bool) = match module_result {
        Ok(r) => (r, false),
        Err(e) => (e, true),
    };

    // —— 写回响应 ——
    let mut resp = json_response(
        StatusCode::from_u16(module_resp.status).unwrap_or(StatusCode::OK),
        &module_resp.body,
    );
    if is_cors_path {
        apply_cors(resp.headers_mut(), &cors_origin);
    }
    // ssa-code 透传
    for (k, v) in &module_resp.headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(v),
        ) {
            resp.headers_mut().insert(name, val);
        }
    }
    // 模块 Set-Cookie
    if !no_cookie {
        for c in &module_resp.cookie {
            set_cookie_headers.push(format!("{}{}", c, cookie_suffix));
        }
    }
    append_set_cookies(resp.headers_mut(), &set_cookie_headers);

    resp
}

// ---------------- 辅助 ----------------

fn apply_cors(headers: &mut HeaderMap, origin: &str) {
    let sets = [
        ("access-control-allow-credentials", "true"),
        ("access-control-allow-origin", origin),
        (
            "access-control-allow-headers",
            "Authorization,X-Requested-With,Content-Type,Cache-Control",
        ),
        ("access-control-allow-methods", "PUT,POST,GET,DELETE,OPTIONS"),
    ];
    for (k, v) in sets {
        if let (Ok(name), Ok(val)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v))
        {
            headers.insert(name, val);
        }
    }
}

fn append_set_cookies(headers: &mut HeaderMap, cookies: &[String]) {
    for c in cookies {
        if let Ok(val) = HeaderValue::from_str(c) {
            headers.append(axum::http::header::SET_COOKIE, val);
        }
    }
}

fn json_response(status: StatusCode, body: &Value) -> AxumResponse {
    let text = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());
    let mut resp = AxumResponse::new(axum::body::Body::from(text));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    resp
}

/// 解析 Cookie 头 -> map
fn parse_cookie_header(headers: &HeaderMap) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let raw = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    for pair in raw.split(';') {
        let crack = pair.find('=');
        if let Some(idx) = crack {
            if idx < 1 || idx == pair.len() - 1 {
                continue;
            }
            let k = pair[..idx].trim().to_string();
            let v = pair[idx + 1..].trim().to_string();
            out.insert(k, v);
        }
    }
    out
}

/// 解析 query string -> (params map, 可选 cookie 对象)
/// params 值一律为字符串（与 Express req.query 一致）；cookie 参数被单独抽出并解析为对象。
fn parse_query(qs: &str) -> (Map<String, Value>, Option<Map<String, Value>>) {
    let mut params = Map::new();
    let mut cookie_obj: Option<Map<String, Value>> = None;
    if qs.is_empty() {
        return (params, cookie_obj);
    }
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_str::<Vec<(String, String)>>(qs).unwrap_or_default();
    for (k, v) in pairs {
        if k == "cookie" {
            cookie_obj = Some(cookie_to_json(&v));
        } else {
            params.insert(k, Value::String(v));
        }
    }
    (params, cookie_obj)
}

/// 解析请求体：json / urlencoded / 原始文本
fn parse_body(headers: &HeaderMap, body: &Bytes) -> Value {
    if body.is_empty() {
        return Value::Object(Map::new());
    }
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if ct.contains("application/json") {
        serde_json::from_slice::<Value>(body).unwrap_or(Value::Object(Map::new()))
    } else if ct.contains("application/x-www-form-urlencoded") {
        let s = String::from_utf8_lossy(body);
        let pairs: Vec<(String, String)> =
            serde_urlencoded::from_str::<Vec<(String, String)>>(&s).unwrap_or_default();
        let mut m = Map::new();
        for (k, v) in pairs {
            m.insert(k, Value::String(v));
        }
        Value::Object(m)
    } else {
        // 尝试 JSON，否则空对象
        serde_json::from_slice::<Value>(body).unwrap_or(Value::Object(Map::new()))
    }
}
