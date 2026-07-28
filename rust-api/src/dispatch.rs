//! 进程内直调分发器 —— 供 Tauri `invoke` 使用，跳过 HTTP 层。
//!
//! 语义对齐 server.rs 的 handle_request：
//! - 平台 Cookie 注入（KUGOU_API_PLATFORM/MID/GUID/DEV/MAC）
//! - Authorization 字符串解析（与前端 axios 拦截器格式一致：`k=v;k=v`）
//! - query 合并：{ cookie:{...jar, ...queryCookie, ...auth}, ...params, body }
//! - GET 响应缓存 120s（对应 apicache，仅缓存 200）
//! - 模块 Set-Cookie 落入进程内 cookie jar（invoke 模式下没有浏览器帮忙存）
//!
//! 与 HTTP 模式的差异：cookie 不再依赖 WebView 的 cookie 存储，而是由
//! `ApiDispatcher` 内部 jar 持久化（宿主可选择将 jar 序列化落盘）。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::modules::{self, UseAxios};
use crate::request::RequestOptions;
use crate::server::AppState;
use crate::util::{calculate_mid, cookie_to_json};

/// invoke 返回给前端的响应体
#[derive(Debug, Clone, Serialize)]
pub struct DispatchResponse {
    pub status: u16,
    pub body: Value,
    /// 模块下发的 cookie（`k=v` 原样），前端可按需镜像；jar 已自动持久
    #[serde(rename = "setCookies")]
    pub set_cookies: Vec<String>,
    /// ssa-code 等透传头
    pub headers: HashMap<String, String>,
}

const CACHE_DURATION_SECS: u64 = 120;

/// 进程内分发器（一个应用一个实例，由宿主托管）
pub struct ApiDispatcher {
    state: Arc<AppState>,
    /// 进程内 cookie jar（对应浏览器 cookie 存储）
    jar: RwLock<HashMap<String, String>>,
    cache: moka::future::Cache<String, Arc<DispatchResponse>>,
}

impl ApiDispatcher {
    pub fn new() -> Self {
        ApiDispatcher {
            state: Arc::new(AppState::new()),
            jar: RwLock::new(HashMap::new()),
            cache: moka::future::Cache::builder()
                .time_to_live(std::time::Duration::from_secs(CACHE_DURATION_SECS))
                .build(),
        }
    }

    /// 导出 jar（宿主可持久化到磁盘）
    pub fn export_jar(&self) -> HashMap<String, String> {
        self.jar.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// 导入 jar（应用启动时恢复）
    pub fn import_jar(&self, cookies: HashMap<String, String>) {
        if let Ok(mut g) = self.jar.write() {
            for (k, v) in cookies {
                g.insert(k, v);
            }
        }
    }

    /// 清空 jar（退出登录），并同时失效响应缓存
    pub fn clear_jar(&self) {
        if let Ok(mut g) = self.jar.write() {
            g.clear();
        }
        self.cache.invalidate_all();
    }

    /// 统一分发入口。
    /// - `method`: GET/POST/...（仅影响缓存策略）
    /// - `path`: 允许携带内联 query string，如 `/search?keywords=x`
    /// - `params`: 前端 params 对象（值将按 Express req.query 语义字符串化）
    /// - `body`: 请求体（POST json），无则传 Null
    /// - `auth`: 前端拦截器同款 Authorization 字符串（`token=..;userid=..`），可空
    pub async fn dispatch(
        &self,
        method: &str,
        path: &str,
        params: Value,
        body: Value,
        auth: Option<String>,
    ) -> DispatchResponse {
        let method_upper = method.to_ascii_uppercase();

        // —— 拆出内联 query string ——
        let (route, inline_qs) = match path.split_once('?') {
            Some((p, qs)) => (p.to_string(), qs.to_string()),
            None => (path.to_string(), String::new()),
        };

        // —— 合并 params：内联 qs + params 对象（后者优先） ——
        let mut merged: Map<String, Value> = Map::new();
        let mut query_cookie: Option<Map<String, Value>> = None;
        if !inline_qs.is_empty() {
            let pairs: Vec<(String, String)> =
                serde_urlencoded::from_str::<Vec<(String, String)>>(&inline_qs).unwrap_or_default();
            for (k, v) in pairs {
                if k == "cookie" {
                    query_cookie = Some(cookie_to_json(&v));
                } else {
                    merged.insert(k, Value::String(v));
                }
            }
        }
        if let Value::Object(obj) = params {
            for (k, v) in obj {
                if k == "cookie" {
                    if let Value::String(s) = &v {
                        query_cookie = Some(cookie_to_json(s));
                        continue;
                    }
                }
                // Express req.query 语义：标量一律字符串
                let sv = match v {
                    Value::String(s) => Value::String(s),
                    Value::Number(n) => Value::String(n.to_string()),
                    Value::Bool(b) => Value::String(b.to_string()),
                    Value::Null => continue,
                    other => other,
                };
                merged.insert(k, sv);
            }
        }

        // —— 缓存查询（对应 apicache：仅 GET，键含全部参数） ——
        let cache_key = if method_upper == "GET" {
            let mut key = format!("{}|{}", route, serde_json::to_string(&merged).unwrap_or_default());
            if let Some(a) = &auth {
                key.push('|');
                key.push_str(a);
            }
            Some(key)
        } else {
            None
        };
        if let Some(key) = &cache_key {
            if let Some(entry) = self.cache.get(key).await {
                return (*entry).clone();
            }
        }

        // —— 平台 Cookie 注入（对应 server.rs ensure 逻辑） ——
        let mut set_cookies: Vec<String> = Vec::new();
        {
            let platform = std::env::var("platform").unwrap_or_default();
            let guid_env =
                std::env::var("KUGOU_API_GUID").unwrap_or_else(|_| self.state.guid.clone());
            let mid = calculate_mid(&guid_env);
            let dev = std::env::var("KUGOU_API_DEV")
                .unwrap_or_else(|_| self.state.server_dev.clone())
                .to_uppercase();
            let mac = std::env::var("KUGOU_API_MAC")
                .unwrap_or_else(|_| "02:00:00:00:00:00".to_string())
                .to_uppercase();

            if let Ok(mut jar) = self.jar.write() {
                let mut ensure = |key: &str, value: String| {
                    if !jar.contains_key(key) {
                        jar.insert(key.to_string(), value.clone());
                        set_cookies.push(format!("{}={}", key, value));
                    }
                };
                ensure("KUGOU_API_PLATFORM", platform);
                ensure("KUGOU_API_MID", mid);
                ensure("KUGOU_API_GUID", guid_env);
                ensure("KUGOU_API_DEV", dev);
                ensure("KUGOU_API_MAC", mac);
            }
        }

        // —— cookie 对象：{...jar, ...queryCookie, ...auth} ——
        let mut cookie_obj: Map<String, Value> = Map::new();
        if let Ok(jar) = self.jar.read() {
            for (k, v) in jar.iter() {
                cookie_obj.insert(k.clone(), Value::String(v.clone()));
            }
        }
        if let Some(qc) = query_cookie {
            for (k, v) in qc {
                cookie_obj.insert(k, v);
            }
        }
        if let Some(a) = &auth {
            if !a.is_empty() {
                for (k, v) in cookie_to_json(a) {
                    cookie_obj.insert(k, v);
                }
            }
        }

        // —— query = { cookie, ...params, body } ——
        let mut query = Map::new();
        query.insert("cookie".into(), Value::Object(cookie_obj));
        for (k, v) in merged {
            query.insert(k, v);
        }
        query.insert(
            "body".into(),
            if body.is_null() { Value::Object(Map::new()) } else { body },
        );

        let no_cookie = query
            .get("noCookie")
            .map(|v| !matches!(v, Value::Null) && v.as_str() != Some("0") && v.as_bool() != Some(false))
            .unwrap_or(false);

        // —— 模块分发 ——
        let module_fn = match modules::lookup(&route) {
            Some(f) => f,
            None => {
                return DispatchResponse {
                    status: 404,
                    body: serde_json::json!({ "code": 404, "data": null, "msg": "Not Found" }),
                    set_cookies,
                    headers: HashMap::new(),
                };
            }
        };

        let use_axios: UseAxios = Arc::new(move |mut opts: RequestOptions| {
            opts.ip = Some(String::new());
            Box::pin(crate::request::create_request(opts))
        });

        let module_result = module_fn(Value::Object(query), use_axios).await;

        // Err 且 body 为空 -> 404 信封（对齐 server.rs）
        if let Err(ref e) = module_result {
            if e.body.is_null() {
                return DispatchResponse {
                    status: 404,
                    body: serde_json::json!({ "code": 404, "data": null, "msg": "Not Found" }),
                    set_cookies,
                    headers: HashMap::new(),
                };
            }
        }
        let module_resp = match module_result {
            Ok(r) => r,
            Err(e) => e,
        };

        // —— 模块 Set-Cookie 落 jar ——
        if !no_cookie {
            if let Ok(mut jar) = self.jar.write() {
                for c in &module_resp.cookie {
                    if let Some((k, v)) = c.split_once('=') {
                        jar.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    set_cookies.push(c.clone());
                }
            }
        }

        let resp = DispatchResponse {
            status: module_resp.status,
            body: module_resp.body,
            set_cookies,
            headers: module_resp.headers.into_iter().collect(),
        };

        // —— 仅缓存 200 的 GET ——
        if resp.status == 200 {
            if let Some(key) = cache_key {
                self.cache.insert(key, Arc::new(resp.clone())).await;
            }
        }

        resp
    }
}

impl Default for ApiDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
