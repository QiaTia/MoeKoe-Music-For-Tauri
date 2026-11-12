//! WebSocket 远程控制服务（ws://127.0.0.1:6520），
//! 复刻 electron/services/apiService.js：welcome / lyrics / playerState 广播 + control 指令。

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::Response,
    routing::any,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Clone)]
struct WsShared {
    app: AppHandle,
    clients: Arc<Mutex<HashMap<u64, UnboundedSender<String>>>>,
    lyrics: Arc<Mutex<Value>>,
    playing: Arc<Mutex<(bool, f64, f64)>>, // (isPlaying, currentTime, duration)
    next_id: Arc<AtomicU64>,
}

/// WS 服务配置（对应 v1.7.2 的 `apiModeConfig`：监听主机 / 端口 / WS 路径）
#[derive(Clone, Debug, PartialEq)]
pub struct WsConfig {
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Default for WsConfig {
    fn default() -> Self {
        WsConfig {
            host: "127.0.0.1".to_string(),
            port: 6520,
            path: "/moekoe".to_string(),
        }
    }
}

impl WsConfig {
    /// 从 settings.apiModeConfig 读取；字段非法时逐项回退默认值。
    /// 默认值与 v1.7.2 客户端内置 API 一致（127.0.0.1:6520 + /moekoe）。
    pub fn from_settings(store: &crate::settings::SettingsStore) -> Self {
        let cfg = store
            .settings()
            .get("apiModeConfig")
            .cloned()
            .unwrap_or(Value::Null);
        WsConfig::from_value(&cfg)
    }

    /// 纯函数版本：便于单测（`apiModeConfig` 的 JSON 形态直接喂入）
    pub fn from_value(cfg: &Value) -> Self {
        let d = WsConfig::default();
        let host = cfg
            .get("host")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|s| {
                // IPv6 字面量需要方括号才能参与 "host:port" 拼接
                if s.contains(':') && !s.starts_with('[') {
                    format!("[{}]", s)
                } else {
                    s
                }
            })
            .unwrap_or(d.host.clone());

        // 端口可能被设置页以字符串形式写入，两种形态都要兼容
        let port = match cfg.get("port") {
            Some(Value::Number(n)) => n.as_u64(),
            Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
            _ => None,
        }
        .filter(|p| (1..=65535).contains(p))
        .map(|p| p as u16)
        .unwrap_or(d.port);

        let path = cfg
            .get("wsPath")
            .and_then(|v| v.as_str())
            .map(|s| WsConfig::sanitize_path(s))
            .unwrap_or(d.path);

        WsConfig { host, port, path }
    }

    /// 校验 WS 路径，非法时回退默认 `/moekoe`。
    ///
    /// axum 的 `Router::route` 对含空格、`{`、`*` 等字符的路径会 panic，
    /// 而用户可在设置页自由填写 `wsPath`，因此必须先做白名单收敛。
    fn sanitize_path(raw: &str) -> String {
        let s = raw.trim();
        if s.is_empty() {
            return WsConfig::default().path;
        }
        let s = if s.starts_with('/') {
            s.to_string()
        } else {
            format!("/{}", s)
        };
        let ok = s.len() <= 65
            && s.chars().skip(1).all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.')
            });
        if ok {
            s
        } else {
            WsConfig::default().path
        }
    }
}

/// apiMode 三态判定。
///
/// - `on`   → 启动 WS 服务
/// - `auto` → 对齐 v1.7.2 语义：仅当存在需要 `moekoe:apiMode` 权限的扩展时才启动；
///            Tauri 版未内置扩展体系，故等价于不启动
/// - `off`  → 不启动
pub fn api_mode_should_start(store: &crate::settings::SettingsStore) -> bool {
    matches!(store.setting_str("apiMode").as_deref(), Some("on"))
}

/// 同步 bind（带短暂重试）。
///
/// `stop()` 只是 abort 任务，内核对监听 socket 的释放可能滞后一拍，
/// 紧接着按同一端口重新 bind 会偶发 `AddrInUse`；仅在地址被占用时重试。
fn bind_with_retry(host: &str, port: u16) -> Result<std::net::TcpListener, String> {
    const ATTEMPTS: usize = 5;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
    let addr = format!("{}:{}", host, port);
    let mut last_err = String::new();
    for attempt in 0..ATTEMPTS {
        match std::net::TcpListener::bind(&addr) {
            Ok(listener) => {
                listener
                    .set_nonblocking(true)
                    .map_err(|e| format!("{} 设置非阻塞失败: {e}", addr))?;
                return Ok(listener);
            }
            Err(e) => {
                last_err = e.to_string();
                if e.kind() != std::io::ErrorKind::AddrInUse {
                    break;
                }
                if attempt + 1 < ATTEMPTS {
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }
    Err(format!("{} 监听失败: {}", addr, last_err))
}

/// WS 服务托管（可随 apiMode 设置启停）
pub struct WsService {
    inner: Mutex<Option<WsRuntime>>,
    config: Mutex<Option<WsConfig>>,
    lyrics: Arc<Mutex<Value>>,
    playing: Arc<Mutex<(bool, f64, f64)>>,
    current_song: Arc<Mutex<Value>>,
    clients: Arc<Mutex<HashMap<u64, UnboundedSender<String>>>>,
}

struct WsRuntime {
    handle: tauri::async_runtime::JoinHandle<()>,
}

impl WsService {
    pub fn new() -> Self {
        WsService {
            inner: Mutex::new(None),
            config: Mutex::new(None),
            lyrics: Arc::new(Mutex::new(Value::Null)),
            playing: Arc::new(Mutex::new((false, 0.0, 0.0))),
            current_song: Arc::new(Mutex::new(Value::Null)),
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 当前运行配置（未运行时为 None）
    pub fn current_config(&self) -> Option<WsConfig> {
        self.config.lock().ok().and_then(|c| c.clone())
    }

    /// 启动 WS 服务。
    ///
    /// 采用「同步 bind + 异步 serve」：bind 失败立刻返回 Err，调用方与
    /// `is_running()` 都能得到真实状态（旧实现把 bind 放在 spawn 内，
    /// 端口被占用时 `is_running()` 仍是 true，UI 显示已开启却根本没监听）。
    pub fn start(&self, app: AppHandle, cfg: WsConfig) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "WS 状态锁已损坏".to_string())?;
        if guard.is_some() {
            return Ok(()); // 已在运行
        }

        let cfg = WsConfig {
            path: WsConfig::sanitize_path(&cfg.path),
            ..cfg
        };
        let std_listener = bind_with_retry(&cfg.host, cfg.port)?;

        let shared = WsShared {
            app,
            clients: self.clients.clone(),
            lyrics: self.lyrics.clone(),
            playing: self.playing.clone(),
            next_id: Arc::new(AtomicU64::new(1)),
        };

        let advertised = format!("ws://{}:{}{}", cfg.host, cfg.port, cfg.path);
        let ws_path = cfg.path.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[ws] 监听句柄转换失败: {e}");
                    return;
                }
            };
            let mut router = Router::new().route("/", any(ws_handler));
            // 配置路径与根路径同时可连：既兼容 v1.7.2 的 /moekoe 约定，也不破坏旧客户端
            if ws_path != "/" {
                router = router.route(&ws_path, any(ws_handler));
            }
            let router = router.with_state(shared);
            println!("WebSocket server running at {}", advertised);
            let _ = axum::serve(listener, router).await;
        });

        *guard = Some(WsRuntime { handle });
        drop(guard);
        if let Ok(mut c) = self.config.lock() {
            *c = Some(cfg);
        }
        Ok(())
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(rt) = guard.take() {
                rt.handle.abort();
            }
        }
        if let Ok(mut c) = self.config.lock() {
            *c = None;
        }
        if let Ok(mut clients) = self.clients.lock() {
            clients.clear();
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn broadcast(&self, value: &Value) {
        let text = value.to_string();
        if let Ok(clients) = self.clients.lock() {
            for tx in clients.values() {
                let _ = tx.send(text.clone());
            }
        }
    }

    /// server-lyrics → 记录并广播
    pub fn update_lyrics(&self, lyrics: Value) {
        if let Ok(mut g) = self.lyrics.lock() {
            *g = lyrics.clone();
        }
        self.broadcast(&json!({ "type": "lyrics", "data": lyrics }));
    }

    /// play-pause-action → 记录并广播播放状态
    pub fn update_player_state(&self, is_playing: bool, current_time: f64) {
        if let Ok(mut g) = self.playing.lock() {
            // 保留已有的 duration，仅更新 is_playing 和 current_time
            *g = (is_playing, current_time, g.2);
        }
        self.broadcast(&json!({
            "type": "playerState",
            "data": { "isPlaying": is_playing, "currentTime": current_time }
        }));
    }

    /// player-state → 记录完整播放状态（含 duration）
    pub fn update_player_full_state(&self, is_playing: bool, current_time: f64, duration: f64) {
        if let Ok(mut g) = self.playing.lock() {
            *g = (is_playing, current_time, duration);
        }
        self.broadcast(&json!({
            "type": "playerState",
            "data": { "isPlaying": is_playing, "currentTime": current_time, "duration": duration }
        }));
    }

    /// 主窗 → 记录当前歌曲信息（供小屏播放器初始同步）
    pub fn update_current_song(&self, song: Value) {
        if let Ok(mut g) = self.current_song.lock() {
            *g = song.clone();
        }
    }

    /// 取当前播放状态快照（歌曲信息 + 播放态 + 歌词），用于小屏播放器初始同步
    pub fn snapshot(&self) -> (Value, (bool, f64, f64), Value) {
        let song = self.current_song.lock().map(|g| g.clone()).unwrap_or(Value::Null);
        let playing = self.playing.lock().map(|g| *g).unwrap_or((false, 0.0, 0.0));
        let lyrics = self.lyrics.lock().map(|g| g.clone()).unwrap_or(Value::Null);
        (song, playing, lyrics)
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(shared): State<WsShared>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, shared))
}

async fn handle_socket(socket: WebSocket, shared: WsShared) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = unbounded_channel::<String>();
    let id = shared.next_id.fetch_add(1, Ordering::Relaxed);

    // 欢迎信息 + 当前状态（对齐 apiService）
    let _ = tx.send(
        json!({
            "type": "welcome",
            "data": "感谢接入MoeKoe Music，文档地址：https://music.moekoe.cn/"
        })
        .to_string(),
    );
    if let Ok(lyrics) = shared.lyrics.lock() {
        if !lyrics.is_null() {
            let _ = tx.send(json!({ "type": "lyrics", "data": lyrics.clone() }).to_string());
        }
    }
    if let Ok(playing) = shared.playing.lock() {
        let _ = tx.send(
            json!({
                "type": "playerState",
                "data": { "isPlaying": playing.0, "currentTime": playing.1, "duration": playing.2 }
            })
            .to_string(),
        );
    }

    if let Ok(mut clients) = shared.clients.lock() {
        clients.insert(id, tx);
    }

    // 出站泵
    let send_task = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    // 入站：control 指令 → 转发主窗口
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Text(text) = msg {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if value.get("type").and_then(|v| v.as_str()) == Some("control") {
                    let command = value
                        .get("data")
                        .and_then(|d| d.get("command"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let channel = match command {
                        "toggle" => Some("toggle-play-pause"),
                        "next" => Some("play-next-track"),
                        "prev" => Some("play-previous-track"),
                        _ => None,
                    };
                    if let Some(channel) = channel {
                        let _ = shared.app.emit_to(crate::windows::MAIN, channel, ());
                    }
                }
            }
        }
    }

    // 断开清理
    if let Ok(mut clients) = shared.clients.lock() {
        clients.remove(&id);
    }
    send_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_config_defaults_when_missing() {
        let cfg = WsConfig::from_value(&Value::Null);
        assert_eq!(
            cfg,
            WsConfig {
                host: "127.0.0.1".into(),
                port: 6520,
                path: "/moekoe".into(),
            }
        );
    }

    #[test]
    fn ws_config_accepts_string_port_and_normalizes_path() {
        // 设置页可能把端口写成字符串；路径缺少前导斜杠要补上
        let cfg = WsConfig::from_value(&json!({
            "host": " 0.0.0.0 ",
            "port": "16520",
            "wsPath": "custom"
        }));
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 16520);
        assert_eq!(cfg.path, "/custom");
    }

    #[test]
    fn ws_config_falls_back_on_invalid_values() {
        // 端口越界 / 空白 host 都必须回退默认，不能把非法值传给 bind
        let cfg = WsConfig::from_value(&json!({
            "host": "   ",
            "port": 70000,
            "wsPath": ""
        }));
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 6520);
        assert_eq!(cfg.path, "/moekoe");
    }

    #[test]
    fn ws_config_brackets_ipv6_host() {
        let cfg = WsConfig::from_value(&json!({ "host": "::1" }));
        assert_eq!(cfg.host, "[::1]");
        // 已带方括号的不重复包裹
        let cfg = WsConfig::from_value(&json!({ "host": "[::1]" }));
        assert_eq!(cfg.host, "[::1]");
    }

    #[test]
    fn ws_path_rejects_axum_panic_patterns() {
        // axum 的 Router::route 遇到空格 / { } / * 会 panic，必须回退默认路径
        for bad in ["/a b", "/{x}", "/a*b", "/中文"] {
            assert_eq!(WsConfig::sanitize_path(bad), "/moekoe", "非法路径 {} 未回退", bad);
        }
        assert_eq!(WsConfig::sanitize_path("/moekoe"), "/moekoe");
        assert_eq!(WsConfig::sanitize_path("/"), "/");
        assert_eq!(WsConfig::sanitize_path("a_b-c.d/1"), "/a_b-c.d/1");
        // 超长路径同样回退
        let long = format!("/{}", "a".repeat(80));
        assert_eq!(WsConfig::sanitize_path(&long), "/moekoe");
    }
}
