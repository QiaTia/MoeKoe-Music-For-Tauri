//! MoeKoe Music Tauri 宿主。
//!
//! - `api_request`：前端 invoke 直调 Rust 版酷狗 API（进程内分发，不经 HTTP）
//! - `ipc_send`：Electron IPC 兼容层（托盘/歌词窗/设置/快捷键等 15+ 通道）
//! - 系统集成：托盘、全局快捷键、单实例、moekoe:// 深链、开机自启、在线更新、WS 远程控制

mod ipc;
mod settings;
mod shortcuts;
mod statusbar;
mod tray;
mod updater;
mod windows;
mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use kugoumusicapi::dispatch::{ApiDispatcher, DispatchResponse};
use serde_json::{json, Value};
use settings::SettingsStore;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg(target_os = "windows")]
pub const PLATFORM: &str = "win32";
#[cfg(target_os = "macos")]
pub const PLATFORM: &str = "darwin";
#[cfg(target_os = "linux")]
pub const PLATFORM: &str = "linux";

/// 全局应用上下文
pub struct AppCtx {
    pub dispatcher: Arc<ApiDispatcher>,
    pub jar_path: RwLock<Option<PathBuf>>,
    pub settings: SettingsStore,
    pub ws: ws::WsService,
    /// 冷启动时通过 moekoe:// 深链传入的参数（等待前端就绪后取走）
    pub startup_url_params: Mutex<Option<Value>>,
}

impl AppCtx {
    fn save_jar(&self) {
        let path = match self.jar_path.read().ok().and_then(|g| g.clone()) {
            Some(p) => p,
            None => return,
        };
        let jar = self.dispatcher.export_jar();
        if let Ok(text) = serde_json::to_string(&jar) {
            let _ = std::fs::write(path, text);
        }
    }

    fn load_jar(&self) {
        let path = match self.jar_path.read().ok().and_then(|g| g.clone()) {
            Some(p) => p,
            None => return,
        };
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(jar) = serde_json::from_str::<HashMap<String, String>>(&text) {
                self.dispatcher.import_jar(jar);
            }
        }
    }
}

// ─────────────────────────── API 命令 ───────────────────────────

/// 前端统一 API 调用入口（酷狗 API 进程内直调）
#[tauri::command]
async fn api_request(
    state: tauri::State<'_, AppCtx>,
    method: String,
    path: String,
    params: Option<Value>,
    body: Option<Value>,
    auth: Option<String>,
) -> Result<DispatchResponse, String> {
    let resp = state
        .dispatcher
        .dispatch(
            &method,
            &path,
            params.unwrap_or(Value::Null),
            body.unwrap_or(Value::Null),
            auth,
        )
        .await;
    if !resp.set_cookies.is_empty() {
        state.save_jar();
    }
    Ok(resp)
}

/// 查看当前 cookie jar（调试/迁移用）
#[tauri::command]
fn api_export_cookies(state: tauri::State<'_, AppCtx>) -> HashMap<String, String> {
    state.dispatcher.export_jar()
}

/// 清空 cookie jar（退出登录）
#[tauri::command]
fn api_clear_cookies(state: tauri::State<'_, AppCtx>) {
    state.dispatcher.clear_jar();
    let path = state.jar_path.read().ok().and_then(|g| g.clone());
    if let Some(p) = path {
        let _ = std::fs::remove_file(p);
    }
}

/// 听歌识曲：接收前端录制的原始 PCM（16bit）音频，调用酷狗指纹服务识别歌曲。
/// 二进制 body 无法走通用 `api_request`（JSON 序列化会破坏原始字节），故独立实现：
/// 复用进程内 cookie jar，直接调用 `create_request` 并以 `sign_binary` 注入流式 Android 签名。
#[tauri::command]
async fn recognize_audio(
    state: tauri::State<'_, AppCtx>,
    pcm: Vec<u8>,
) -> Result<Value, String> {
    // 复用与 api_request 同一套鉴权上下文（含登录态 cookie）
    let jar = state.dispatcher.export_jar();
    let mut cookie: serde_json::Map<String, Value> = serde_json::Map::new();
    for (k, v) in jar {
        cookie.insert(k, Value::String(v));
    }
    let opts = kugoumusicapi::modules::build_audio_match_options(pcm, cookie);
    let resp = kugoumusicapi::request::create_request(opts).await;
    // 无论成功或将被拒，都取回 body 透传给前端（前端按 {status, data} 解析）
    let body = match resp {
        Ok(r) => r.body,
        Err(r) => r.body,
    };
    Ok(body)
}

// ─────────────────────────── 深链处理 ───────────────────────────

/// moekoe://share?hash=..&listid=.. | moekoe://team/join?code=.. → {hash, listid, teamcode, urlPath}
fn parse_deep_link(url_str: &str) -> Option<Value> {
    let url = url::Url::parse(url_str).ok()?;
    if url.scheme() != "moekoe" {
        return None;
    }
    let mut hash = String::new();
    let mut listid = String::new();
    let mut teamcode = String::new();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "hash" => hash = v.to_string(),
            "listid" => listid = v.to_string(),
            "code" => teamcode = v.to_string(),
            _ => {}
        }
    }
    // Electron: urlPath = pathname 去掉前导斜杠；冷启动回放时固定为 share（组队为 join）
    let mut url_path = url.path().trim_start_matches('/').to_string();
    if url_path.is_empty() {
        url_path = url
            .host_str()
            .unwrap_or(if teamcode.is_empty() { "share" } else { "join" })
            .to_string();
    }
    Some(json!({ "hash": hash, "listid": listid, "teamcode": teamcode, "urlPath": url_path }))
}

fn handle_deep_link(app: &tauri::AppHandle, url_str: &str, cold_start: bool) {
    let Some(params) = parse_deep_link(url_str) else {
        return;
    };
    if cold_start {
        let ctx = app.state::<AppCtx>();
        if let Ok(mut g) = ctx.startup_url_params.lock() {
            *g = Some(params);
        };
    } else {
        let _ = app.emit_to(windows::MAIN, "url-params", params);
    }
}

// ─────────────────────────── 入口 ───────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 与 Node/独立 bin 版保持一致的平台参数（--platform=lite 对应 concept 数据源）
    if std::env::var("platform").is_err() {
        std::env::set_var("platform", "lite");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // 第二实例：唤起主窗口 + 处理深链参数（对齐 second-instance）
            if let Some(win) = app.get_webview_window(windows::MAIN) {
                if win.is_minimized().unwrap_or(false) {
                    let _ = win.unminimize();
                }
                let _ = win.show();
                let _ = win.set_focus();
            }
            for arg in argv {
                if arg.starts_with("moekoe:") {
                    handle_deep_link(app, &arg, false);
                }
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppCtx {
            dispatcher: Arc::new(ApiDispatcher::new()),
            jar_path: RwLock::new(None),
            settings: SettingsStore::new(),
            ws: ws::WsService::new(),
            startup_url_params: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let ctx = app.state::<AppCtx>();

            // 1. 存储初始化
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
                ctx.settings.init(dir.join("store.json"));
                if let Ok(mut g) = ctx.jar_path.write() {
                    *g = Some(dir.join("cookie_jar.json"));
                }
                ctx.load_jar();
            }

            // 2. 冷启动深链参数（Windows/Linux 从 argv 拿）
            for arg in std::env::args().skip(1) {
                if arg.starts_with("moekoe:") {
                    handle_deep_link(&handle, &arg, true);
                }
            }
            // 深链协议注册（dev 环境）+ 运行期回调（macOS open-url 等价物）
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                let _ = app.deep_link().register_all();
            }
            {
                let handle2 = handle.clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        handle_deep_link(&handle2, url.as_str(), false);
                    }
                });
            }

            // 3. 主窗口 + 托盘 + 快捷键
            windows::create_main_window(&handle, &ctx.settings)?;
            tray::create_tray(&handle)?;
            shortcuts::register_all(&handle);

            // 4. 桌面歌词窗（按设置恢复）
            if ctx.settings.setting_on("desktopLyrics") {
                let _ = windows::create_lyrics_window(&handle, &ctx.settings);
            }

            // 5. WS 远程控制（apiMode）
            if ctx.settings.setting_on("apiMode") {
                ctx.ws.start(handle.clone());
            }

            // 6. 启动问候语
            ipc::play_startup_sound(&handle);

            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match (window.label(), event) {
                // 主窗口关闭 → 最小化到托盘 or 退出（对齐 electron close 拦截）
                (windows::MAIN, tauri::WindowEvent::CloseRequested { api, .. }) => {
                    let ctx = app.state::<AppCtx>();
                    let minimize_to_tray =
                        ctx.settings.setting_str("minimizeToTray").as_deref() != Some("off");
                    if minimize_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        windows::save_main_window_state(app, &ctx.settings);
                        ctx.ws.stop();
                        app.exit(0);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            api_request,
            api_export_cookies,
            api_clear_cookies,
            recognize_audio,
            ipc::ipc_send,
            ipc::app_boot_info,
            ipc::open_mv_window,
            ipc::open_log_path,
            ipc::export_log,
            ipc::start_update_download,
            ipc::ipc_invoke,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
