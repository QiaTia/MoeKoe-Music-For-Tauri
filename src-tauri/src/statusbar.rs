//! macOS 菜单栏歌词：复用 Tauri 的 TrayIcon 在系统状态栏承载滚动歌词图片。
//!
//! 前端 `StatusBarLyrics.vue` 把 Canvas 渲染好的 PNG 通过 `update-statusbar-image`
//! 通道推过来；本模块解码 base64 并设置到状态栏 tray 图标（彩色，非模板）。
//!
//! 仅 macOS 真正生效：设置项 `statusBarLyrics` 本就只 `darwin` 可用，且为「重启后生效」。
//! 非 macOS 平台编译为空实现，不影响其它平台。

pub const STATUSBAR_TRAY_ID: &str = "statusbar-lyrics";

#[cfg(target_os = "macos")]
mod imp {
    use super::STATUSBAR_TRAY_ID;
    use std::sync::Mutex;

    use base64::Engine;
    use tauri::{
        image::Image,
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        AppHandle, Manager,
    };

    // 记录上一次设置的图片 dataURL，避免静态歌词时每 200ms 重复 set_icon
    static LAST: Mutex<Option<String>> = Mutex::new(None);

    pub(super) fn set_statusbar_image(app: &AppHandle, data: &str) {
        if data == "clear" {
            remove_statusbar(app);
            return;
        }

        // 去重：与上次完全相同的帧直接跳过（静态短歌词时每 200ms 重发同一图）
        if let Ok(mut g) = LAST.lock() {
            if g.as_deref() == Some(data) {
                return;
            }
            *g = Some(data.to_string());
        }

        let b64 = match strip_data_url(data) {
            Some(b) => b,
            None => return,
        };
        let bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[statusbar] base64 解码失败: {e}");
                return;
            }
        };
        let img = match Image::from_bytes(&bytes) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("[statusbar] Image::from_bytes 失败: {e}");
                return;
            }
        };

        if let Some(tray) = app.tray_by_id(STATUSBAR_TRAY_ID) {
            let _ = tray.set_icon(Some(img));
        } else {
            // 首次创建：不带 menu 的状态栏项，左键点击聚焦主窗口
            let builder = TrayIconBuilder::with_id(STATUSBAR_TRAY_ID)
                .icon(img)
                .icon_as_template(false)
                .tooltip("MoeKoe Music")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(win) = tray
                            .app_handle()
                            .get_webview_window(crate::windows::MAIN)
                        {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                });
            if let Err(e) = builder.build(app) {
                eprintln!("[statusbar] 创建状态栏歌词 tray 失败: {e}");
            }
        }
    }

    pub(super) fn remove_statusbar(app: &AppHandle) {
        if let Ok(mut g) = LAST.lock() {
            *g = None;
        }
        let _ = app.remove_tray_by_id(STATUSBAR_TRAY_ID);
    }

    /// 去掉 `data:image/png;base64,` 前缀，返回纯 base64 串；
    /// 若已是裸 base64 则原样返回。
    fn strip_data_url(data: &str) -> Option<&str> {
        if let Some(rest) = data.strip_prefix("data:image/png;base64,") {
            Some(rest)
        } else if data.starts_with("data:") {
            data.split_once(',').map(|(_, b)| b)
        } else {
            Some(data)
        }
    }
}

#[cfg(target_os = "macos")]
pub fn set_statusbar_image(app: &AppHandle, data: &str) {
    imp::set_statusbar_image(app, data);
}

#[cfg(target_os = "macos")]
pub fn remove_statusbar(app: &AppHandle) {
    imp::remove_statusbar(app);
}

#[cfg(not(target_os = "macos"))]
pub fn set_statusbar_image(_app: &AppHandle, _data: &str) {}

#[cfg(not(target_os = "macos"))]
pub fn remove_statusbar(_app: &AppHandle) {}
