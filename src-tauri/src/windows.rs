//! 窗口管理：主窗口 / 桌面歌词窗 / MV 窗。
//! 语义对齐 electron/appServices.js 的 createWindow / createLyricsWindow / createMvWindow。

use crate::settings::SettingsStore;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const MAIN: &str = "main";
pub const LYRICS: &str = "lyrics";
pub const MV: &str = "mv";

fn primary_work_area(app: &AppHandle) -> (f64, f64) {
    // 逻辑尺寸（DIP），与 electron screen.workAreaSize 对齐
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        (size.width, size.height)
    } else {
        (1920.0, 1080.0)
    }
}

/// 创建主窗口（按保存的 windowState 恢复位置尺寸）
pub fn create_main_window(app: &AppHandle, store: &SettingsStore) -> tauri::Result<WebviewWindow> {
    // macOS 始终使用原生标题栏（提供红绿灯按钮），其他平台按用户设置
    #[cfg(target_os = "macos")]
    let native_title_bar = true;
    #[cfg(not(target_os = "macos"))]
    let native_title_bar = store.setting_str("nativeTitleBar").as_deref() == Some("on");
    let start_minimized = store.setting_str("startMinimized").as_deref() == Some("on");
    let (screen_w, screen_h) = primary_work_area(app);

    let default_w = 1200f64.min(screen_w * 0.8);
    let default_h = 938f64.min(screen_h * 0.9);

    let state = store.get("windowState");
    let mut width = state.get("width").and_then(|v| v.as_f64()).unwrap_or(default_w);
    let mut height = state.get("height").and_then(|v| v.as_f64()).unwrap_or(default_h);
    width = width.min(screen_w);
    height = height.min(screen_h);

    let x = state.get("x").and_then(|v| v.as_f64());
    let y = state.get("y").and_then(|v| v.as_f64());
    let valid_pos = matches!((x, y), (Some(px), Some(py))
        if px >= 0.0 && px <= screen_w && py >= 0.0 && py <= screen_h);

    let mut builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("MoeKoe Music")
        .inner_size(width, height)
        .min_inner_size(890.0, 750.0)
        .decorations(native_title_bar)
        .visible(!start_minimized)
        .disable_drag_drop_handler(); // 允许页面内 HTML5 拖拽

    // Windows WebView2: 允许混合内容（HTTPS 页面加载 HTTP 图片等资源）
    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args("--allow-mixed-content");
    }

    if valid_pos {
        builder = builder.position(x.unwrap(), y.unwrap());
    } else {
        builder = builder.center();
    }

    if store.get("maximize").as_bool().unwrap_or(false) {
        builder = builder.maximized(true);
    }

    let window = builder.build()?;

    // 仅当显式开启 devtools 特性时自动打开（release 包默认无此特性，右键不会再有 Inspect 菜单）
    #[cfg(feature = "devtools")]
    window.open_devtools();

    Ok(window)
}

/// 保存主窗口位置尺寸（对齐 before-quit 的 windowState 持久化）
pub fn save_main_window_state(app: &AppHandle, store: &SettingsStore) {
    if let Some(win) = app.get_webview_window(MAIN) {
        if win.is_maximized().unwrap_or(false) {
            return;
        }
        let scale = win.scale_factor().unwrap_or(1.0);
        if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
            let pos = pos.to_logical::<f64>(scale);
            let size = size.to_logical::<f64>(scale);
            store.set(
                "windowState",
                json!({ "x": pos.x, "y": pos.y, "width": size.width, "height": size.height }),
            );
        }
    }
}

/// 创建桌面歌词窗口（透明 / 置顶 / 不进任务栏）
pub fn create_lyrics_window(app: &AppHandle, store: &SettingsStore) -> tauri::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(LYRICS) {
        return Ok(existing);
    }

    let (screen_w, screen_h) = primary_work_area(app);
    let default_w = (screen_w * 0.7).floor();
    let default_h = 200.0;

    let saved_pos = store.get("lyricsWindowPosition");
    let saved_size = store.get("lyricsWindowSize");

    let mut width = saved_size.get("width").and_then(|v| v.as_f64()).unwrap_or(default_w);
    let mut height = saved_size.get("height").and_then(|v| v.as_f64()).unwrap_or(default_h);
    width = width.min(screen_w);
    height = height.min(screen_h);

    let x = saved_pos.get("x").and_then(|v| v.as_f64());
    let y = saved_pos.get("y").and_then(|v| v.as_f64());
    let valid_pos = matches!((x, y), (Some(px), Some(py))
        if px >= 0.0 && px <= screen_w && py >= 0.0 && py <= screen_h);

    let (px, py) = if valid_pos {
        (x.unwrap(), y.unwrap())
    } else {
        (((screen_w - width) / 2.0).floor(), screen_h - height)
    };

    let builder =
        WebviewWindowBuilder::new(app, LYRICS, WebviewUrl::App("index.html#/lyrics".into()))
            .title("MoeKoe Lyrics")
            .inner_size(width, height)
            .min_inner_size(800.0, 200.0)
            .position(px, py)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .resizable(true);

    // Windows WebView2: 允许混合内容
    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args("--allow-mixed-content");

    let window = builder.build()?;

    // 尺寸变化持久化（对齐 electron 'resize' → lyricsWindowSize）
    {
        let app = app.clone();
        let win = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Resized(size) = event {
                let scale = win.scale_factor().unwrap_or(1.0);
                let logical: LogicalSize<f64> = size.to_logical(scale);
                let store = app.state::<crate::AppCtx>();
                store.settings.set(
                    "lyricsWindowSize",
                    json!({ "width": logical.width, "height": logical.height }),
                );
            }
        });
    }

    // Lyrics.vue onMounted 会立即 setIgnoreMouseEvents(true)，先置初始状态并启动 forward 轮询
    LYRICS_IGNORING.store(true, Ordering::Relaxed);
    spawn_lyrics_forward_thread(app);

    Ok(window)
}

/// 歌词窗当前是否处于"忽略鼠标事件"状态（由前端 set-ignore-mouse-events 驱动）。
/// Electron 的 setIgnoreMouseEvents(true, { forward: true }) 在忽略点击的同时仍会把
/// mousemove 转发给页面；Tauri 的 set_ignore_cursor_events 没有 forward 能力，
/// 因此用一个光标轮询线程来模拟：忽略状态下检测到光标进入歌词窗范围时，
/// 将坐标以 shim-mousemove 事件发给前端，由 tauriShim 合成真实 mousemove 派发。
pub static LYRICS_IGNORING: AtomicBool = AtomicBool::new(true);
static FORWARD_RUNNING: AtomicBool = AtomicBool::new(false);

fn spawn_lyrics_forward_thread(app: &AppHandle) {
    if FORWARD_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return; // 已有线程在跑
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut was_inside = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(33));
            let Some(win) = app.get_webview_window(LYRICS) else { break };
            let (Ok(cursor), Ok(pos), Ok(size)) =
                (app.cursor_position(), win.outer_position(), win.outer_size())
            else {
                continue;
            };
            let scale = win.scale_factor().unwrap_or(1.0);
            let inside = cursor.x >= pos.x as f64
                && cursor.x < (pos.x as f64 + size.width as f64)
                && cursor.y >= pos.y as f64
                && cursor.y < (pos.y as f64 + size.height as f64);

            if inside && LYRICS_IGNORING.load(Ordering::Relaxed) {
                // 转发窗口内坐标（逻辑像素），页面据此做悬停检测
                let _ = app.emit_to(
                    LYRICS,
                    "shim-mousemove",
                    json!({
                        "x": (cursor.x - pos.x as f64) / scale,
                        "y": (cursor.y - pos.y as f64) / scale,
                        "sx": cursor.x / scale,
                        "sy": cursor.y / scale,
                    }),
                );
            } else if !inside && was_inside {
                // 光标离开窗口：补发一次窗外坐标，让页面重置悬停并恢复穿透
                let _ = app.emit_to(
                    LYRICS,
                    "shim-mousemove",
                    json!({ "x": -9999.0, "y": -9999.0, "sx": -9999.0, "sy": -9999.0 }),
                );
            }
            was_inside = inside;
        }
        FORWARD_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// 关闭桌面歌词窗口，返回是否真的关了
pub fn close_lyrics_window(app: &AppHandle) -> bool {
    if let Some(win) = app.get_webview_window(LYRICS) {
        let _ = win.close();
        true
    } else {
        false
    }
}

/// 移动歌词窗口（渲染进程手动拖拽 / 缩放）
/// width/height 为可选项：纯拖拽移动时与当前尺寸一致（set_size 为空操作）；
/// 边缘缩放时下发新尺寸。坐标均为屏幕逻辑像素。
pub fn move_lyrics_window(
    app: &AppHandle,
    store: &SettingsStore,
    x: f64,
    y: f64,
    width: Option<f64>,
    height: Option<f64>,
) {
    if let Some(win) = app.get_webview_window(LYRICS) {
        let _ = win.set_position(LogicalPosition::new(x, y));
        if let (Some(w), Some(h)) = (width, height) {
            let _ = win.set_size(LogicalSize::new(w, h));
        }
        store.set("lyricsWindowPosition", json!({ "x": x, "y": y }));
    }
}

/// 创建 MV 窗口加载指定 URL
pub fn create_mv_window(app: &AppHandle, url: &str) -> Result<(), String> {
    // 已存在则复用导航
    if let Some(win) = app.get_webview_window(MV) {
        let _ = win.close();
    }

    let parsed: url::Url = url.parse().map_err(|e| format!("invalid url: {e}"))?;
    let (screen_w, screen_h) = primary_work_area(app);

    let builder = WebviewWindowBuilder::new(app, MV, WebviewUrl::External(parsed))
        .title("MoeKoe MV")
        .inner_size((screen_w * 0.8).min(1280.0), (screen_h * 0.8).min(720.0))
        .decorations(false)
        .transparent(true)
        .center();

    // Windows WebView2: 允许混合内容
    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args("--allow-mixed-content");

    let window = builder.build().map_err(|e| e.to_string())?;

    let _ = window.show();
    Ok(())
}
