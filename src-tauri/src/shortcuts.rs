//! 全局快捷键：Electron accelerator 字符串 → global-hotkey 格式转换 + 注册。
//! 对齐 electron/appServices.js registerShortcut 的默认键位与自定义逻辑。

use crate::windows::{LYRICS, MAIN};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// 快捷键动作
#[derive(Clone, Copy, Debug)]
enum Action {
    Quit,
    ToggleMainWindow,
    EmitMain(&'static str),
    ToggleDesktopLyrics,
}

/// (settings.shortcuts 键名, 默认 accelerator, 动作)
const BINDINGS: &[(&str, &str, Action)] = &[
    ("quitApp", "CmdOrCtrl+Q", Action::Quit),
    ("mainWindow", "CmdOrCtrl+Shift+S", Action::ToggleMainWindow),
    ("prevTrack", "Alt+CommandOrControl+Left", Action::EmitMain("play-previous-track")),
    ("nextTrack", "Alt+CommandOrControl+Right", Action::EmitMain("play-next-track")),
    ("volumeUp", "Alt+CommandOrControl+Up", Action::EmitMain("volume-up")),
    ("volumeDown", "Alt+CommandOrControl+Down", Action::EmitMain("volume-down")),
    ("playPause", "Alt+CommandOrControl+Space", Action::EmitMain("toggle-play-pause")),
    ("mute", "Alt+CommandOrControl+M", Action::EmitMain("toggle-mute")),
    ("like", "Alt+CommandOrControl+L", Action::EmitMain("toggle-like")),
    ("mode", "Alt+CommandOrControl+P", Action::EmitMain("toggle-mode")),
    ("toggleDesktopLyrics", "Alt+Ctrl+D", Action::ToggleDesktopLyrics),
];

/// Electron accelerator → global-hotkey 可解析格式
/// 例：`Alt+CommandOrControl+Left` → `Alt+CmdOrCtrl+ArrowLeft`；`Ctrl+Alt+D` → `Ctrl+Alt+KeyD`
pub fn convert_accelerator(acc: &str) -> Option<String> {
    let parts: Vec<&str> = acc.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut mods: Vec<String> = Vec::new();
    let mut key: Option<String> = None;

    for part in &parts {
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "commandorcontrol" | "cmdorctrl" => mods.push("CmdOrCtrl".into()),
            "command" | "cmd" | "super" | "meta" => mods.push("Super".into()),
            "control" | "ctrl" => mods.push("Ctrl".into()),
            "alt" | "option" | "altgr" => mods.push("Alt".into()),
            "shift" => mods.push("Shift".into()),
            _ => {
                // 键位转换
                let converted = match lower.as_str() {
                    "left" => "ArrowLeft".to_string(),
                    "right" => "ArrowRight".to_string(),
                    "up" => "ArrowUp".to_string(),
                    "down" => "ArrowDown".to_string(),
                    "space" | "spacebar" => "Space".to_string(),
                    "esc" | "escape" => "Escape".to_string(),
                    "return" | "enter" => "Enter".to_string(),
                    "tab" => "Tab".to_string(),
                    "backspace" => "Backspace".to_string(),
                    "delete" | "del" => "Delete".to_string(),
                    "home" => "Home".to_string(),
                    "end" => "End".to_string(),
                    "pageup" => "PageUp".to_string(),
                    "pagedown" => "PageDown".to_string(),
                    "plus" => "Equal".to_string(),
                    "minus" | "-" => "Minus".to_string(),
                    "," => "Comma".to_string(),
                    "." => "Period".to_string(),
                    s if s.len() == 1 && s.chars().all(|c| c.is_ascii_alphabetic()) => {
                        format!("Key{}", s.to_ascii_uppercase())
                    }
                    s if s.len() == 1 && s.chars().all(|c| c.is_ascii_digit()) => {
                        format!("Digit{s}")
                    }
                    s if s.starts_with('f')
                        && s[1..].parse::<u8>().map(|n| (1..=24).contains(&n)).unwrap_or(false) =>
                    {
                        s.to_ascii_uppercase()
                    }
                    _ => part.to_string(), // 交给解析器兜底（如 MediaPlayPause）
                };
                key = Some(converted);
            }
        }
    }

    let key = key?;
    if mods.is_empty() {
        Some(key)
    } else {
        Some(format!("{}+{}", mods.join("+"), key))
    }
}

fn run_action(app: &AppHandle, action: Action) {
    match action {
        Action::Quit => crate::tray::quit_app(app),
        Action::ToggleMainWindow => {
            if let Some(win) = app.get_webview_window(MAIN) {
                if win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                } else {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        }
        Action::EmitMain(channel) => {
            let _ = app.emit_to(MAIN, channel, ());
        }
        Action::ToggleDesktopLyrics => {
            let ctx = app.state::<crate::AppCtx>();
            if app.get_webview_window(LYRICS).is_some() {
                crate::windows::close_lyrics_window(app);
                crate::ipc::notify_lyrics_closed(app);
                ctx.settings.merge_setting("desktopLyrics", "off".into());
            } else {
                let _ = crate::windows::create_lyrics_window(app, &ctx.settings);
                ctx.settings.merge_setting("desktopLyrics", "on".into());
            }
        }
    }
}

/// 注册全部快捷键（先清空再注册），对齐 registerShortcut 语义：
/// - settings.shortcuts 不存在 → 全部注册默认键
/// - settings.shortcuts 存在 → 仅注册用户配置了的键
pub fn register_all(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let ctx = app.state::<crate::AppCtx>();
    let shortcuts_cfg = ctx.settings.settings().get("shortcuts").cloned().unwrap_or(Value::Null);
    let has_custom = shortcuts_cfg.is_object();

    for (name, default_acc, action) in BINDINGS {
        // macOS 上 quitApp 不注册全局键（对齐 electron）
        #[cfg(target_os = "macos")]
        if *name == "quitApp" {
            continue;
        }

        let acc: Option<String> = if has_custom {
            shortcuts_cfg
                .get(*name)
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
        } else {
            Some((*default_acc).to_string())
        };

        let Some(acc) = acc else { continue };
        let Some(converted) = convert_accelerator(&acc) else {
            eprintln!("[shortcuts] 无法解析快捷键: {acc}");
            continue;
        };

        let action = *action;
        let result = gs.on_shortcut(converted.as_str(), move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                run_action(app, action);
            }
        });
        if let Err(e) = result {
            eprintln!("[shortcuts] 注册失败 {name} ({converted}): {e}");
        }
    }
}
