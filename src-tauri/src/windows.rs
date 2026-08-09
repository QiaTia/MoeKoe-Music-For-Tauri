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
///
/// 修复：
/// - `.decorations(true)` 恢复原生标题栏 → macOS/Windows 上有原生关闭按钮（之前 frameless 导致 Mac 无法关闭）
/// - `.transparent(false)` → 之前 transparent=true 在 WebView2 播放视频后关闭会产生透明黑框残留
/// 从 MV 内部路由 URL 的 fragment query 中解析视频标题，例如
/// `http://localhost:8080/#/video?hash=..&title=视频名`。
/// 解析失败则回退到固定标题，避免原生标题栏为空。
fn mv_window_title(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.fragment().map(|f| f.to_string()))
        .and_then(|f| url::Url::parse(&format!("http://localhost/{f}")).ok())
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "title")
                .map(|(_, v)| v.to_string())
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "MoeKoe MV".to_string())
}

/// - 已存在窗口改用 `navigate` 而非 close+rebuild，避免同一 label 旧窗口未释放完就建新的导致竞态残影
pub fn create_mv_window(app: &AppHandle, url: &str) -> Result<(), String> {
    let parsed: url::Url = url.parse().map_err(|e| format!("invalid url: {e}"))?;
    let title = mv_window_title(url);

    // 已存在则就地导航并聚焦（避免 close+rebuild 的销毁竞态）
    if let Some(win) = app.get_webview_window(MV) {
        let _ = win.navigate(parsed);
        let _ = win.set_title(&title);
        let _ = win.set_focus();
        return Ok(());
    }

    let (screen_w, screen_h) = primary_work_area(app);

    let builder = WebviewWindowBuilder::new(app, MV, WebviewUrl::External(parsed))
        .title("MoeKoe MV")
        .inner_size((screen_w * 0.8).min(1280.0), (screen_h * 0.8).min(720.0))
        .decorations(true)
        .transparent(false)
        .center();

    // Windows WebView2: 允许混合内容
    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args("--allow-mixed-content");

    let window = builder.build().map_err(|e| e.to_string())?;

    let _ = window.set_title(&title);
    let _ = window.show();
    Ok(())
}

/// ─────────────────────────── 小屏/迷你播放器窗口 ───────────────────────────
/// 方案 A（独立小窗）：加载 /mini 路由，无边框、透明、置顶、跳过任务栏。
/// 与桌面歌词窗机制一致，但承载完整播放控制与增强功能（歌词浮层/取色背景/边缘吸附）。

pub const MINI: &str = "mini";
pub const MINI_MODE_NORMAL: &str = "normal";
pub const MINI_MODE_COMPACT: &str = "compact";

/// 不同形态对应的窗口尺寸（逻辑像素）
fn mini_size_for_mode(mode: &str) -> (f64, f64) {
    match mode {
        MINI_MODE_COMPACT => (320.0, 80.0),
        _ => (320.0, 460.0),
    }
}

/// 合并写入 miniWindowState 的部分字段（避免整体替换丢失其它字段）
fn patch_mini_window_state(store: &SettingsStore, patch: serde_json::Value) {
    let mut state = store.get("miniWindowState");
    if !state.is_object() {
        state = json!({});
    }
    if let Some(obj) = state.as_object_mut() {
        if let Some(p) = patch.as_object() {
            for (k, v) in p {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    store.set("miniWindowState", state);
}

/// 边缘吸附阈值：窗口任一边距屏幕工作区边缘小于该值（逻辑像素）即吸附。
const MINI_SNAP_THRESHOLD: f64 = 16.0;
/// 吸附后窗口边缘距屏幕工作区边缘的留白。
const MINI_EDGE_MARGIN: f64 = 12.0;
/// 防止 `set_position` 触发的二次 `Moved` 重入造成抖动/死循环。
static MINI_SNAPPING: AtomicBool = AtomicBool::new(false);

/// 边缘吸附增强：小窗松手（WindowEvent::Moved）后，若任一边距当前显示器工作区边缘
/// 小于阈值，则把该边吸附到边缘（留 `MINI_EDGE_MARGIN`）。水平/竖直方向独立判定，
/// 因此可吸附到屏幕角。窗口已离开边缘（>阈值）时不吸附，避免误触。
/// 多显示器下使用窗口所在显示器的 work_area（避开任务栏），否则回退主屏。
fn snap_mini_window_if_needed(app: &AppHandle, win: &WebviewWindow) {
    // 已是重入（本函数 set_position 触发的二次 Moved）：放行但不再吸附
    if MINI_SNAPPING.swap(true, Ordering::SeqCst) {
        MINI_SNAPPING.store(false, Ordering::SeqCst);
        return;
    }

    let scale = win.scale_factor().unwrap_or(1.0);
    let pos = match win.outer_position() {
        Ok(p) => p.to_logical::<f64>(scale),
        Err(_) => {
            MINI_SNAPPING.store(false, Ordering::SeqCst);
            return;
        }
    };
    let size = match win.inner_size() {
        Ok(s) => s.to_logical::<f64>(scale),
        Err(_) => {
            MINI_SNAPPING.store(false, Ordering::SeqCst);
            return;
        }
    };

    // 工作区（含多屏全局偏移）：优先窗口所在显示器，回退主屏
    let (work_x, work_y, work_w, work_h) = match win.current_monitor() {
        Ok(Some(m)) => {
            let ms = m.scale_factor();
            let wa = m.work_area();
            let wp = wa.position.to_logical::<f64>(ms);
            let ws = wa.size.to_logical::<f64>(ms);
            (wp.x, wp.y, ws.width, ws.height)
        }
        _ => {
            let (w, h) = primary_work_area(app);
            (0.0, 0.0, w, h)
        }
    };

    let wx = pos.x;
    let wy = pos.y;
    let w = size.width;
    let h = size.height;

    let left = wx - work_x;
    let right = (work_x + work_w) - (wx + w);
    let top = wy - work_y;
    let bottom = (work_y + work_h) - (wy + h);

    let mut nx = wx;
    let mut ny = wy;

    // 水平方向：吸附到更近的一侧
    if left < MINI_SNAP_THRESHOLD && left <= right {
        nx = work_x + MINI_EDGE_MARGIN;
    } else if right < MINI_SNAP_THRESHOLD {
        nx = work_x + work_w - w - MINI_EDGE_MARGIN;
    }
    // 竖直方向：吸附到更近的一侧
    if top < MINI_SNAP_THRESHOLD && top <= bottom {
        ny = work_y + MINI_EDGE_MARGIN;
    } else if bottom < MINI_SNAP_THRESHOLD {
        ny = work_y + work_h - h - MINI_EDGE_MARGIN;
    }

    // 夹紧到工作区内，保证窗口至少部分可见（多屏拖出时拉回）
    nx = nx.clamp(work_x, (work_x + work_w - w).max(work_x));
    ny = ny.clamp(work_y, (work_y + work_h - h).max(work_y));

    if (nx, ny) != (wx, wy) {
        let _ = win.set_position(LogicalPosition::new(nx, ny));
        let ctx = app.state::<crate::AppCtx>();
        let store = &ctx.settings;
        patch_mini_window_state(store, json!({ "x": nx, "y": ny }));
    }

    MINI_SNAPPING.store(false, Ordering::SeqCst);
}

/// 所有显示器的「安全工作区」矩形（逻辑像素，含多屏全局偏移）。
/// 优先用 work_area（避开任务栏/Dock），无显示器信息时回退主屏。
fn visible_monitor_rects(app: &AppHandle) -> Vec<(f64, f64, f64, f64)> {
    let mut rects = Vec::new();
    if let Ok(monitors) = app.available_monitors() {
        for m in monitors {
            let scale = m.scale_factor();
            let wa = m.work_area();
            let p = wa.position.to_logical::<f64>(scale);
            let s = wa.size.to_logical::<f64>(scale);
            rects.push((p.x, p.y, s.width, s.height));
        }
    }
    if rects.is_empty() {
        let (w, h) = primary_work_area(app);
        rects.push((0.0, 0.0, w, h));
    }
    rects
}

/// 显示器配置指纹：任一显示器插拔/分辨率/缩放变更都会改变它，用于检测变化。
fn monitor_signature(app: &AppHandle) -> String {
    let mut parts = Vec::new();
    if let Ok(monitors) = app.available_monitors() {
        for m in monitors {
            let scale = m.scale_factor();
            let p = m.position().to_logical::<f64>(scale);
            let s = m.size().to_logical::<f64>(scale);
            parts.push(format!(
                "{:.2},{:.2},{:.2},{:.2},{:.2}",
                p.x, p.y, s.width, s.height, scale
            ));
        }
    }
    parts.sort();
    parts.join("|")
}

/// 求「离 (x,y) 最近、且能使窗口整体落在某显示器内」的安全位置。
/// 若窗口已整体在某显示器内，直接返回原位置（不打扰用户）。
fn nearest_safe_position(
    rects: &[(f64, f64, f64, f64)],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> (f64, f64) {
    for &(mx, my, mw, mh) in rects {
        if x >= mx && y >= my && x + w <= mx + mw && y + h <= my + mh {
            return (x, y);
        }
    }
    let mut best: Option<((f64, f64), f64)> = None;
    for &(mx, my, mw, mh) in rects {
        let (nx, ny) = if mw >= w && mh >= h {
            (
                x.clamp(mx, (mx + mw - w).max(mx)),
                y.clamp(my, (my + mh - h).max(my)),
            )
        } else {
            // 窗口比显示器还大：尽量居中兜底
            (
                mx + (mw - w).max(0.0) / 2.0,
                my + (mh - h).max(0.0) / 2.0,
            )
        };
        let dist = ((nx - x).powi(2) + (ny - y).powi(2)).sqrt();
        if best.as_ref().map_or(true, |(_, d)| dist < *d) {
            best = Some(((nx, ny), dist));
        }
    }
    best.map(|(p, _)| p).unwrap_or((x, y))
}

/// 安全校正：把小窗位置拉回最近的安全可见区域（仅当窗口整体已不可见时才移动），
/// 并在确有移动时持久化新坐标，避免下次又从越界位置打开。
fn ensure_mini_visible(app: &AppHandle, win: &WebviewWindow) {
    let scale = win.scale_factor().unwrap_or(1.0);
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
        let pos = pos.to_logical::<f64>(scale);
        let size = size.to_logical::<f64>(scale);
        let rects = visible_monitor_rects(app);
        let (nx, ny) = nearest_safe_position(&rects, pos.x, pos.y, size.width, size.height);
        if (nx, ny) != (pos.x, pos.y) {
            let _ = win.set_position(LogicalPosition::new(nx, ny));
            let ctx = app.state::<crate::AppCtx>();
            let store = &ctx.settings;
            patch_mini_window_state(store, json!({ "x": nx, "y": ny }));
        }
    }
}

/// 显示器配置变化监控（单例线程）：分辨率变更 / 拔出副屏后，
/// 若小窗被甩到屏幕外，自动拉回最近的安全区域。小窗关闭即退出，下次打开再拉起。
static MINI_MONITOR_WATCH: AtomicBool = AtomicBool::new(false);

fn start_mini_monitor_watch(app: &AppHandle) {
    if MINI_MONITOR_WATCH
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return; // 已有监控线程在跑
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut last_sig = monitor_signature(&app);
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            // 小窗暂时关闭：保持监控待命，等待它再次出现。
            // 不退出线程，避免「关闭→立刻重开」的竞态导致重开后丢失监控。
            let Some(win) = app.get_webview_window(MINI) else {
                continue;
            };
            let sig = monitor_signature(&app);
            if sig != last_sig {
                last_sig = sig;
                ensure_mini_visible(&app, &win);
            }
        }
    });
}

pub fn create_mini_window(app: &AppHandle, store: &SettingsStore) -> tauri::Result<WebviewWindow> {
    // 已存在则聚焦，避免重复创建
    if let Some(existing) = app.get_webview_window(MINI) {
        let _ = existing.show();
        let _ = existing.set_focus();
        // 显示器变化可能使已存在的小窗落到屏外，打开时先校正一次
        ensure_mini_visible(app, &existing);
        start_mini_monitor_watch(app);
        start_mini_sync_timer(app);
        // 小屏模式始终隐藏主窗口
        if let Some(main) = app.get_webview_window(MAIN) {
            let _ = main.hide();
        }
        // 直接向小窗推送当前播放状态（避免主窗往返的时序问题）
        push_mini_state(app);
        return Ok(existing);
    }

    let (screen_w, screen_h) = primary_work_area(app);
    let state = store.get("miniWindowState");
    // 读取持久化记忆（Rust 端在 toggle/set_mode 时已写入 miniWindowState.mode）：
    // 打开时沿用上一次关闭时的 normal/compact，而不是每次都强制成固定形态。
    // 仅当字段缺失或非法（既非 normal 也非 compact）时回退 normal，避免历史污染。
    let mode = state
        .get("mode")
        .and_then(|v| v.as_str())
        .filter(|m| *m == MINI_MODE_NORMAL || *m == MINI_MODE_COMPACT)
        .unwrap_or(MINI_MODE_NORMAL);
    let (width, height) = mini_size_for_mode(mode);

    // 默认位置：屏幕右下角，避免与主窗口重叠
    let x = state
        .get("x")
        .and_then(|v| v.as_f64())
        .unwrap_or(screen_w - width - 24.0);
    let y = state
        .get("y")
        .and_then(|v| v.as_f64())
        .unwrap_or(screen_h - height - 24.0);

    // 把当前持久化形态带进 URL query，前端可在首屏同步渲染出正确布局（消除事件到达前的闪烁）
    let mini_url = format!("index.html#/mini?mode={}", mode);
    let mut builder = WebviewWindowBuilder::new(app, MINI, WebviewUrl::App(mini_url.into()))
        .title("MoeKoe Mini")
        .inner_size(width, height)
        .min_inner_size(280.0, 60.0)
        .max_inner_size(400.0, 460.0)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .visible(true);

    // macOS 用原生阴影更精致；Windows 透明窗 + 阴影会出现黑色矩形光晕，故关闭
    #[cfg(target_os = "macos")]
    {
        builder = builder.shadow(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.shadow(false);
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args("--allow-mixed-content");
    }

    let window = builder.build()?;

    // 安全校正：持久化位置可能因显示器插拔/分辨率变更而落在屏幕外，
    // 打开即校正到最近的安全可见区域（已可见则不动）。
    ensure_mini_visible(app, &window);
    // 启动显示器变化监控（单例），变更后自动把小窗拉回可见区域
    start_mini_monitor_watch(app);
    // 启动 Rust 端周期状态推送（macOS 主窗口 hide 后 WKWebView 会被挂起，
    // 主窗口的 setInterval 不会触发，需要 Rust 端代为推送）
    start_mini_sync_timer(app);

    // 创建后立即把当前播放状态直接推给小窗（从应用状态读取，不依赖主窗往返）
    push_mini_state(app);

    // 同步当前形态/位置给小窗，避免只靠 request-mini-state 的时序竞态导致页面与窗口尺寸错配
    let _ = app.emit_to(MINI, "mini-state", store.get("miniWindowState"));

    // 兜底：上面那条 mini-state 在 Webview 首次挂载时经常因竞态漏收，
    // 延迟 ~250ms 再推一次，确保小窗布局与窗口尺寸在首屏就一致。
    let app_delayed = app.clone();
    let mini_state_snapshot = store.get("miniWindowState");
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let _ = app_delayed.emit_to(MINI, "mini-state", mini_state_snapshot);
    });

    // 打开小屏模式后自动隐藏主窗口（网易云小屏模式行为）
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.hide();
    }

    // 主动触发一次状态同步请求，让小窗在创建后立即拿到当前播放信息
    let _ = app.emit_to(MAIN, "request-player-state-sync", ());

    // 拖拽移动 / 形态切换导致的尺寸变化持久化
    {
        let app = app.clone();
        let win = window.clone();
        window.on_window_event(move |event| match event {
            // 原生拖拽由系统处理；窗口移动结束后先做边缘吸附（可能微调位置），
            // 再持久化最终位置（吸附后的实际坐标）
            tauri::WindowEvent::Moved(pos) => {
                let scale = win.scale_factor().unwrap_or(1.0);
                let logical = pos.to_logical::<f64>(scale);
                let ctx = app.state::<crate::AppCtx>();
                let store = &ctx.settings;
                snap_mini_window_if_needed(&app, &win);
                // 读取吸附后的实际位置持久化（吸附已落盘时此处为冗余覆盖，安全）
                if let Ok(p) = win.outer_position() {
                    let lp = p.to_logical::<f64>(scale);
                    patch_mini_window_state(store, json!({ "x": lp.x, "y": lp.y }));
                } else {
                    patch_mini_window_state(store, json!({ "x": logical.x, "y": logical.y }));
                }
            }
            tauri::WindowEvent::Resized(_) => {
                let ctx = app.state::<crate::AppCtx>();
                let store = &ctx.settings;
                if let Ok(size) = win.inner_size() {
                    let scale = win.scale_factor().unwrap_or(1.0);
                    let logical = size.to_logical::<f64>(scale);
                    // 窗口关闭/最小化/透明窗过渡时可能产生 0x0 等异常尺寸，跳过避免污染持久化
                    if logical.width <= 1.0 || logical.height <= 1.0 {
                        return;
                    }
                    // 形态(mode)由 toggle/set_mode 写入，不再按高度反推，
                    // 防止过渡态把 mode 误写成 compact，导致下次打开窗口与页面对不上。
                    patch_mini_window_state(
                        store,
                        json!({ "width": logical.width, "height": logical.height }),
                    );
                    let _ = app.emit_to(MINI, "mini-state", store.get("miniWindowState"));
                }
            }
            _ => {}
        });
    }

    Ok(window)
}

/// 在两种形态间切换 normal <-> compact
pub fn toggle_mini_mode(app: &AppHandle, store: &SettingsStore) {
    let Some(win) = app.get_webview_window(MINI) else {
        return;
    };
    let state = store.get("miniWindowState");
    let current = state
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or(MINI_MODE_NORMAL);
    let next = if current == MINI_MODE_COMPACT {
        MINI_MODE_NORMAL
    } else {
        MINI_MODE_COMPACT
    };
    let (w, h) = mini_size_for_mode(next);
    let _ = win.set_size(LogicalSize::new(w, h));
    patch_mini_window_state(store, json!({ "mode": next }));
    // 切完形态立即通知前端，保持状态同步
    let _ = app.emit_to(MINI, "mini-state", store.get("miniWindowState"));
}

/// 显式设置形态（前端按钮直接指定）
pub fn set_mini_mode(app: &AppHandle, store: &SettingsStore, mode: &str) {
    let Some(win) = app.get_webview_window(MINI) else {
        return;
    };
    let (w, h) = mini_size_for_mode(mode);
    let _ = win.set_size(LogicalSize::new(w, h));
    patch_mini_window_state(store, json!({ "mode": mode }));
    let _ = app.emit_to(MINI, "mini-state", store.get("miniWindowState"));
}

pub fn close_mini_window(app: &AppHandle) -> bool {
    if let Some(win) = app.get_webview_window(MINI) {
        let _ = win.close();
        // 关闭小屏窗口后恢复主窗口
        if let Some(main) = app.get_webview_window(MAIN) {
            let _ = main.show();
            let _ = main.set_focus();
        }
        true
    } else {
        false
    }
}

/// 小屏播放器创建 / 请求同步时，直接从应用状态把当前播放信息推给小窗。
/// 不依赖「小窗 → 主窗 → 小窗」的往返，规避时序与事件丢失问题。
pub fn push_mini_state(app: &AppHandle) {
    if let Some(_win) = app.get_webview_window(MINI) {
        let ws = &app.state::<crate::AppCtx>().ws;
        let (song, playing, lyrics) = ws.snapshot();
        // 定向推送给小窗（避免无谓地广播到主窗）
        let _ = app.emit_to(
            MINI,
            "player-state",
            json!({
                "currentTime": playing.1,
                "duration": playing.2,
                "playing": playing.0,
                "currentSongHash": song.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
            }),
        );
        let _ = app.emit_to(MINI, "player-meta", song);
        if !lyrics.is_null() {
            let _ = app.emit_to(MINI, "lyrics-data", lyrics);
        }
    }
}

/// 乐观推送：toggle-play 时，先从 Rust 缓存读取当前 playing 状态并翻转，
/// 立即推给 Mini 窗口（用户无需等待主窗口 WebView 响应）。
/// 后续由延迟的 push_mini_state 或主窗口 player-state 事件覆盖为真实值。
pub fn push_mini_state_optimistic(app: &AppHandle, toggle: bool) {
    if let Some(_win) = app.get_webview_window(MINI) {
        let ws = &app.state::<crate::AppCtx>().ws;
        let (song, playing, lyrics) = ws.snapshot();
        let optimistic_playing = if toggle { !playing.0 } else { playing.0 };
        let _ = app.emit_to(
            MINI,
            "player-state",
            json!({
                "currentTime": playing.1,
                "duration": playing.2,
                "playing": optimistic_playing,
                "currentSongHash": song.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
            }),
        );
        if !lyrics.is_null() {
            let _ = app.emit_to(MINI, "lyrics-data", lyrics);
        }
    }
}

/// Mini 窗口专用周期状态推送定时器。
/// macOS 主窗口 hide 后 WKWebView 会被系统挂起，主窗口的 setInterval / timeupdate 不会触发，
/// 导致 player-state 事件停止推送到 Mini 窗口。此定时器在 Rust 端直接运行，
/// 不依赖主窗口 WebView，确保 Mini 窗口进度/歌词持续更新。
static MINI_SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start_mini_sync_timer(app: &AppHandle) {
    if MINI_SYNC_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return; // 已有定时器在跑
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            // Mini 窗口不存在时保持线程待命，但不推送
            if app.get_webview_window(MINI).is_none() {
                continue;
            }
            push_mini_state(&app);
        }
        // 理论上不会到这里，线程随进程退出
        // MINI_SYNC_RUNNING.store(false, Ordering::SeqCst);
    });
}
