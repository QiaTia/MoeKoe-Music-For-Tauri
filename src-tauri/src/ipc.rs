//! IPC 兼容层：承接前端 `window.electron.ipcRenderer.send(...)` 的全部通道，
//! 以及 `window.electronAPI.*` 的 invoke 型调用。

use crate::windows::{LYRICS, MAIN, MINI};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, LogicalSize, Manager};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

/// 桌面歌词已关闭通知
pub fn notify_lyrics_closed(app: &AppHandle) {
    let ctx = app.state::<crate::AppCtx>();
    let _ = app
        .notification()
        .builder()
        .title(crate::settings::t(&ctx.settings, "desktop-lyrics-closed"))
        .show();
}

fn arg(args: &Value, i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

/// 统一入口：前端 ipcRenderer.send(channel, ...args) → invoke('ipc_send', {channel, args})
// 必须是 async：Windows 上同步 command 在主线程执行，
// 在其中创建 WebviewWindow（display-lyrics）会与事件循环死锁。
#[tauri::command]
pub async fn ipc_send(app: AppHandle, channel: String, args: Value) {
    let ctx = app.state::<crate::AppCtx>();

    match channel.as_str() {
        // ── 免责声明 ─────────────────────────────────────────────
        "disclaimer-response" => {
            let accepted = arg(&args, 0).as_bool().unwrap_or(false);
            if accepted {
                ctx.settings.set("disclaimerAccepted", json!(true));
            } else {
                crate::tray::quit_app(&app);
            }
        }

        // ── 窗口控制（自绘标题栏） ────────────────────────────────
        "window-control" => {
            let action = arg(&args, 0);
            let action = action.as_str().unwrap_or("");
            if let Some(win) = app.get_webview_window(MAIN) {
                match action {
                    "close" => {
                        let minimize_to_tray =
                            ctx.settings.setting_str("minimizeToTray").as_deref() != Some("off");
                        if minimize_to_tray {
                            let _ = win.hide();
                        } else {
                            crate::tray::quit_app(&app);
                        }
                    }
                    "minimize" => {
                        let _ = win.minimize();
                    }
                    "maximize" => {
                        if win.is_maximized().unwrap_or(false) {
                            let _ = win.unmaximize();
                            ctx.settings.set("maximize", json!(false));
                        } else {
                            let _ = win.maximize();
                            ctx.settings.set("maximize", json!(true));
                        }
                    }
                    _ => {}
                }
            }
        }

        // ── 设置 ─────────────────────────────────────────────────
        "save-settings" => {
            let settings = arg(&args, 0);
            let auto_start = settings.get("autoStart").and_then(|v| v.as_str()).map(|s| s.to_string());
            let api_mode_on = settings.get("apiMode").and_then(|v| v.as_str()) == Some("on");
            // 菜单栏歌词被关闭时，移除系统状态栏项
            let statusbar_lyrics_on =
                settings.get("statusBarLyrics").and_then(|v| v.as_str()) == Some("on");
            // 记录切换前的托盘菜单模式，用于判断是否需要重建托盘
            let old_tray_mode = ctx.settings.setting_str("customTrayMenu");
            ctx.settings.set("settings", settings);
            let new_tray_mode = ctx.settings.setting_str("customTrayMenu");
            if !statusbar_lyrics_on {
                crate::statusbar::remove_statusbar(&app);
            }

            // 开机自启（对齐 setLoginItemSettings）
            if let Some(auto) = auto_start {
                let manager = app.autolaunch();
                if auto == "on" {
                    let _ = manager.enable();
                } else if auto == "off" {
                    let _ = manager.disable();
                }
            }

            // apiMode 热切换 WS 服务
            if api_mode_on && !ctx.ws.is_running() {
                ctx.ws.start(app.clone());
            } else if !api_mode_on && ctx.ws.is_running() {
                ctx.ws.stop();
            }

            // 托盘菜单模式变更：热重建（原生 ↔ 高级）
            if old_tray_mode != new_tray_mode {
                let _ = crate::tray::rebuild_tray(&app);
            }
        }
        "clear-settings" => {
            ctx.settings.clear();
            ctx.dispatcher.clear_jar();
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::remove_file(dir.join("cookie_jar.json"));
                let _ = app
                    .opener()
                    .open_path(dir.to_string_lossy().to_string(), None::<&str>);
            }
        }
        "custom-shortcut" => {
            crate::shortcuts::register_all(&app);
        }

        // ── 歌词链路 ──────────────────────────────────────────────
        "lyrics-data" => {
            // 主窗口 → 桌面歌词窗
            let _ = app.emit_to(LYRICS, "lyrics-data", arg(&args, 0));
            // 小屏播放器歌词浮层
            let _ = app.emit_to(MINI, "lyrics-data", arg(&args, 0));
            // 菜单栏歌词：把当前行文本回发给主窗口的 StatusBarLyrics 渲染成图片
            if let Some(text) = arg(&args, 0).get("currentLyric").and_then(|v| v.as_str()) {
                let _ = app.emit_to(MAIN, "generate-statusbar-image", text.to_string());
            }
        }
        "server-lyrics" => {
            ctx.ws.update_lyrics(arg(&args, 0));
        }
        "desktop-lyrics-action" => {
            let action = arg(&args, 0);
            match action.as_str().unwrap_or("") {
                "previous-song" => {
                    let _ = app.emit_to(MAIN, "play-previous-track", ());
                }
                "next-song" => {
                    let _ = app.emit_to(MAIN, "play-next-track", ());
                }
                "toggle-play" => {
                    let _ = app.emit_to(MAIN, "toggle-play-pause", ());
                }
                "close-lyrics" => {
                    if crate::windows::close_lyrics_window(&app) {
                        notify_lyrics_closed(&app);
                    }
                    ctx.settings.merge_setting("desktopLyrics", "off".into());
                }
                "display-lyrics" => {
                    let _ = crate::windows::create_lyrics_window(&app, &ctx.settings);
                    ctx.settings.merge_setting("desktopLyrics", "on".into());
                }
                _ => {}
            }
        }
        "set-ignore-mouse-events" => {
            let ignore = arg(&args, 0).as_bool().unwrap_or(false);
            crate::windows::LYRICS_IGNORING.store(ignore, std::sync::atomic::Ordering::Relaxed);
            if let Some(win) = app.get_webview_window(LYRICS) {
                let _ = win.set_ignore_cursor_events(ignore);
            }
        }
        "window-drag" => {
            // 前端 Lyrics.vue 的 sendWindowDrag 发送 { x, y, width, height }
            // （屏幕逻辑像素坐标），此处字段名必须与之对齐。
            let pos = arg(&args, 0);
            let x = pos.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = pos.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let width = pos.get("width").and_then(|v| v.as_f64());
            let height = pos.get("height").and_then(|v| v.as_f64());
            crate::windows::move_lyrics_window(&app, &ctx.settings, x, y, width, height);
        }
        "lyrics-window-fixed-size" => {
            // 桌面歌词锁定尺寸时限制最大尺寸（对齐 electron setMaxSize）
            let payload = arg(&args, 0);
            let width = payload.get("width").and_then(|v| v.as_f64());
            let height = payload.get("height").and_then(|v| v.as_f64());
            let fixed = payload.get("fixed").and_then(|v| v.as_bool()).unwrap_or(false);
            if let Some(win) = app.get_webview_window(LYRICS) {
                let max: Option<LogicalSize<f64>> = if fixed {
                    match (width, height) {
                        (Some(w), Some(h)) => Some(LogicalSize::new(w, h)),
                        _ => None,
                    }
                } else {
                    None
                };
                let _ = win.set_max_size(max);
            }
        }

        // ── 播放状态同步 ──────────────────────────────────────────
        "play-pause-action" => {
            let playing = arg(&args, 0).as_bool().unwrap_or(false);
            let current_time = arg(&args, 1).as_f64().unwrap_or(0.0);
            let _ = app.emit_to(LYRICS, "playing-status", playing);
            ctx.ws.update_player_state(playing, current_time);
        }

        // ── 小屏/迷你播放器（独立窗）状态同步 ──────────────────
        "player-state" => {
            // 主窗口周期广播的精简播放状态（currentTime/duration/playing/hash）
            // 仅当迷你窗存在时转发，避免无谓的跨窗口事件
            if app.get_webview_window(MINI).is_some() {
                let _ = app.emit_to(MINI, "player-state", arg(&args, 0));
            }
        }
        "player-meta" => {
            // 记录当前歌曲信息，供小窗初始同步直接读取
            ctx.ws.update_current_song(arg(&args, 0).clone());
            // 切歌时广播的完整 currentSong 对象（name/author/img/hash/quality 等）
            if app.get_webview_window(MINI).is_some() {
                let _ = app.emit_to(MINI, "player-meta", arg(&args, 0));
            }
        }
        "request-player-state-sync" => {
            // 迷你窗打开时请求主窗立即补发一次完整状态
            let _ = app.emit_to(MAIN, "request-player-state-sync", ());
            // 同时直接从应用状态推送，避免主窗往返丢失
            crate::windows::push_mini_state(&app);
        }
        "request-mini-state" => {
            // 迷你窗初始化时获取当前窗口状态（含 mode），用于对齐本地布局
            let _ = app.emit_to(MINI, "mini-state", ctx.settings.get("miniWindowState"));
        }

        // ── 小屏播放器窗口控制 ────────────────────────────────
        "mini-window-control" => {
            let action_val = arg(&args, 0);
            let action = action_val.as_str().unwrap_or("");
            match action {
                "open" => {
                    let _ = crate::windows::create_mini_window(&app, &ctx.settings);
                }
                "close" => {
                    crate::windows::close_mini_window(&app);
                }
                "toggle" => {
                    crate::windows::toggle_mini_mode(&app, &ctx.settings);
                }
                "set-mode" => {
                    let mode_val = arg(&args, 1);
                    let mode = mode_val.as_str().unwrap_or("normal");
                    crate::windows::set_mini_mode(&app, &ctx.settings, mode);
                }
                _ => {}
            }
        }
        "mini-window-drag" => {
            // 把拖拽交给系统原生处理：前端 mousedown 时直接调用，消除 IPC 往返抖动
            if let Some(win) = app.get_webview_window(crate::windows::MINI) {
                let _ = win.start_dragging();
            }
        }

        // ── 杂项 ─────────────────────────────────────────────────
        "open-url" => {
            if let Some(url) = arg(&args, 0).as_str() {
                let _ = app.opener().open_url(url, None::<&str>);
            }
        }
        "set-tray-title" => {
            if let Some(title) = arg(&args, 0).as_str() {
                crate::tray::set_tray_title(&app, title);
            }
        }

        // mac 状态栏歌词：前端 Canvas 图片（PNG dataURL）→ 系统状态栏 tray 图标
        "update-statusbar-image" => {
            if let Some(data) = arg(&args, 0).as_str() {
                crate::statusbar::set_statusbar_image(&app, data);
            }
        }
        // touchBar 通道（仅 macOS Touch Bar 硬件，已无对应设备，静默忽略）
        "update-current-lyrics" => {}

        other => {
            eprintln!("[ipc] 未知通道: {other}");
        }
    }
}

/// 前端启动引导信息（垫片用它合成 version / show-disclaimer / url-params 事件）
#[tauri::command]
pub fn app_boot_info(app: AppHandle) -> Value {
    let ctx = app.state::<crate::AppCtx>();
    let version = app.package_info().version.to_string();
    let show_disclaimer = !ctx.settings.get("disclaimerAccepted").as_bool().unwrap_or(false);
    let url_params = ctx
        .startup_url_params
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .unwrap_or(Value::Null);

    json!({
        "version": version,
        "platform": crate::PLATFORM,
        "showDisclaimer": show_disclaimer,
        "urlParams": url_params,
    })
}

/// 前端 ipcRenderer.invoke(channel, ...args) 的请求/响应通道。
/// Electron 的 invoke 是带返回值的 IPC，对应一个 Rust 命令。
/// 目前仅桌面歌词悬停轮询用到 `lyrics-window-pointer-state`。
#[tauri::command]
pub async fn ipc_invoke(app: AppHandle, channel: String, _args: Value) -> Result<Value, String> {
    match channel.as_str() {
        "lyrics-window-pointer-state" => {
            // 对齐 Electron：screen.getCursorScreenPoint() + win.getBounds()
            // 返回屏幕逻辑像素坐标，供前端 isScreenPointInSelector 做命中检测
            let Some(win) = app.get_webview_window(LYRICS) else {
                return Ok(json!({ "cursor": null, "bounds": null }));
            };
            let scale = win.scale_factor().unwrap_or(1.0);
            let cursor = app.cursor_position().map_err(|e| e.to_string())?;
            let pos = win.outer_position().map_err(|e| e.to_string())?;
            let size = win.outer_size().map_err(|e| e.to_string())?;
            Ok(json!({
                "cursor": {
                    "x": (cursor.x as f64) / scale,
                    "y": (cursor.y as f64) / scale,
                },
                "bounds": {
                    "x": (pos.x as f64) / scale,
                    "y": (pos.y as f64) / scale,
                    "width": (size.width as f64) / scale,
                    "height": (size.height as f64) / scale,
                }
            }))
        }
        other => Err(format!("未知 invoke 通道: {other}")),
    }
}

/// 打开 MV 窗口
#[tauri::command]
pub async fn open_mv_window(app: AppHandle, url: String) -> Result<bool, String> {
    crate::windows::create_mv_window(&app, &url)?;
    Ok(true)
}

/// 打开日志/数据目录
#[tauri::command]
pub fn open_log_path(app: AppHandle) -> Value {
    match app.path().app_data_dir() {
        Ok(dir) => {
            let _ = app
                .opener()
                .open_path(dir.to_string_lossy().to_string(), None::<&str>);
            json!({ "success": true })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// 导出日志（Tauri 版：打开数据目录让用户自取）
#[tauri::command]
pub fn export_log(app: AppHandle) -> Value {
    open_log_path(app)
}

/// 在线更新
#[tauri::command]
pub async fn start_update_download(app: AppHandle) -> Value {
    crate::updater::start_update_download(app).await
}

/// 启动问候语（对齐 playStartupSound）
pub fn play_startup_sound(app: &AppHandle) {
    let ctx = app.state::<crate::AppCtx>();
    let greetings = ctx.settings.setting_str("greetings");
    let settings_exists = ctx.settings.settings().is_object();
    // 对齐 electron：settings 不存在则不播；greetings 为 on/null(字符串) 才播
    if settings_exists && !matches!(greetings.as_deref(), Some("on") | Some("null")) {
        return;
    }
    if !settings_exists {
        return;
    }

    let files = ["yise-jp.mp3", "qiqi-jp.mp3", "qiqi-zh.mp3"];
    let pick = files[std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % files.len()];

    let path = app
        .path()
        .resolve(
            format!("assets/sound/{pick}"),
            tauri::path::BaseDirectory::Resource,
        )
        .ok();
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    // 去掉 Windows 长路径前缀 \\?\，否则 .NET [Uri] 转换失败（InvalidCastFromStringToUri）
    let path_str = path
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();

    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let escaped = path_str.replace('\'', "''");
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-c",
                    &format!(
                        "Add-Type -AssemblyName PresentationCore; $p = New-Object System.Windows.Media.MediaPlayer; $p.Open([Uri]'{escaped}'); $p.Play(); Start-Sleep -s 3; $p.Stop()"
                    ),
                ])
                .creation_flags_hidden()
                .status();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("afplay").arg(&path_str).status();
        }
        #[cfg(target_os = "linux")]
        {
            let ok = std::process::Command::new("paplay")
                .arg(&path_str)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                let _ = std::process::Command::new("play").arg(&path_str).status();
            }
        }
    });
}

#[cfg(target_os = "windows")]
trait HiddenCommand {
    fn creation_flags_hidden(&mut self) -> &mut Self;
}

#[cfg(target_os = "windows")]
impl HiddenCommand for std::process::Command {
    fn creation_flags_hidden(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x08000000) // CREATE_NO_WINDOW
    }
}
