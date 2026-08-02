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
    playing: Arc<Mutex<(bool, f64)>>, // (isPlaying, currentTime)
    next_id: Arc<AtomicU64>,
}

/// WS 服务托管（可随 apiMode 设置启停）
pub struct WsService {
    inner: Mutex<Option<WsRuntime>>,
    lyrics: Arc<Mutex<Value>>,
    playing: Arc<Mutex<(bool, f64)>>,
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
            lyrics: Arc::new(Mutex::new(Value::Null)),
            playing: Arc::new(Mutex::new((false, 0.0))),
            current_song: Arc::new(Mutex::new(Value::Null)),
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&self, app: AppHandle) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.is_some() {
            return; // 已在运行
        }

        let shared = WsShared {
            app,
            clients: self.clients.clone(),
            lyrics: self.lyrics.clone(),
            playing: self.playing.clone(),
            next_id: Arc::new(AtomicU64::new(1)),
        };

        let handle = tauri::async_runtime::spawn(async move {
            let router = Router::new()
                .route("/", any(ws_handler))
                .with_state(shared);
            match tokio::net::TcpListener::bind("127.0.0.1:6520").await {
                Ok(listener) => {
                    println!("WebSocket server running at ws://127.0.0.1:6520");
                    let _ = axum::serve(listener, router).await;
                }
                Err(e) => eprintln!("[ws] 6520 端口监听失败: {e}"),
            }
        });

        *guard = Some(WsRuntime { handle });
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(rt) = guard.take() {
                rt.handle.abort();
            }
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
            *g = (is_playing, current_time);
        }
        self.broadcast(&json!({
            "type": "playerState",
            "data": { "isPlaying": is_playing, "currentTime": current_time }
        }));
    }

    /// 主窗 → 记录当前歌曲信息（供小屏播放器初始同步）
    pub fn update_current_song(&self, song: Value) {
        if let Ok(mut g) = self.current_song.lock() {
            *g = song.clone();
        }
    }

    /// 取当前播放状态快照（歌曲信息 + 播放态 + 歌词），用于小屏播放器初始同步
    pub fn snapshot(&self) -> (Value, (bool, f64), Value) {
        let song = self.current_song.lock().map(|g| g.clone()).unwrap_or(Value::Null);
        let playing = self.playing.lock().map(|g| *g).unwrap_or((false, 0.0));
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
                "data": { "isPlaying": playing.0, "currentTime": playing.1 }
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
