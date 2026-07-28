//! 系统托盘：菜单 + 点击行为，对齐 electron/appServices.js createTray。

use crate::settings::t;
use crate::windows::MAIN;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_opener::OpenerExt;

pub const TRAY_ID: &str = "main-tray";

fn toggle_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN) {
        if win.is_visible().unwrap_or(false) {
            if win.is_focused().unwrap_or(false) {
                let _ = win.hide();
            } else {
                let _ = win.show();
                let _ = win.set_focus();
            }
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

pub fn quit_app(app: &AppHandle) {
    let ctx = app.state::<crate::AppCtx>();
    crate::windows::save_main_window_state(app, &ctx.settings);
    ctx.ws.stop();
    app.exit(0);
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let ctx = app.state::<crate::AppCtx>();
    let store = &ctx.settings;

    let item = |id: &str, key: &str| MenuItemBuilder::with_id(id, t(store, key)).build(app);

    let menu = MenuBuilder::new(app)
        .item(&item("home", "project-home")?)
        .item(&item("bug", "report-bug")?)
        .separator()
        .item(&item("prev", "prev-track")?)
        .item(&item("pause", "pause")?)
        .item(&item("next", "next-track")?)
        .separator()
        .item(&item("update", "check-updates")?)
        .item(&item("restart", "restart-app")?)
        .item(&item("showhide", "show-hide")?)
        .item(&item("quit", "quit")?)
        .build()?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("MoeKoe Music");

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(|app, event| {
            let emit_main = |channel: &str| {
                let _ = app.emit_to(MAIN, channel, ());
            };
            match event.id().as_ref() {
                "home" => {
                    let _ = app
                        .opener()
                        .open_url("https://Music.MoeKoe.cn", None::<&str>);
                }
                "bug" => {
                    let _ = app.opener().open_url(
                        "https://github.com/iAJue/MoeKoeMusic/issues",
                        None::<&str>,
                    );
                }
                "prev" => emit_main("play-previous-track"),
                "pause" => emit_main("toggle-play-pause"),
                "next" => emit_main("play-next-track"),
                "update" => {
                    // 托盘检查更新：打开发布页（前端内亦有完整的在线更新入口）
                    let _ = app.opener().open_url(
                        "https://github.com/iAJue/MoeKoeMusic/releases/latest",
                        None::<&str>,
                    );
                }
                "restart" => {
                    let ctx = app.state::<crate::AppCtx>();
                    crate::windows::save_main_window_state(app, &ctx.settings);
                    app.restart();
                }
                "showhide" => toggle_main_window(app),
                "quit" => quit_app(app),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // 对齐 electron：左键点击显示/聚焦主窗口
                    if let Some(win) = app.get_webview_window(MAIN) {
                        if !win.is_visible().unwrap_or(false) {
                            let _ = win.show();
                            let _ = win.set_focus();
                        } else if !win.is_focused().unwrap_or(false) {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                }
                TrayIconEvent::DoubleClick { .. } => {
                    if let Some(win) = app.get_webview_window(MAIN) {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// set-tray-title：更新托盘 tooltip + 主窗口标题
pub fn set_tray_title(app: &AppHandle, title: &str) {
    let ctx = app.state::<crate::AppCtx>();
    let tooltip = format!("{}{}", t(&ctx.settings, "now-playing"), title);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip));
    }
    if let Some(win) = app.get_webview_window(MAIN) {
        let _ = win.set_title(title);
    }
}
