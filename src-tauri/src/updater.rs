//! 在线更新：GitHub Releases 检查 + 下载（带进度事件），
//! 事件形状对齐 electron-updater：update-progress / update-downloaded / update-error。

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

const RELEASE_API: &str = "https://api.github.com/repos/qiatia/MoeKoe-Music-For-Tauri/releases/latest";

fn version_parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v')
        .split(['.', '-'])
        .filter_map(|p| p.parse::<u64>().ok())
        .collect()
}

fn is_newer(current: &str, latest: &str) -> bool {
    let c = version_parts(current);
    let l = version_parts(latest);
    for i in 0..c.len().max(l.len()) {
        let cv = c.get(i).copied().unwrap_or(0);
        let lv = l.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

fn pick_asset(assets: &[Value]) -> Option<(String, String)> {
    let patterns: &[&str] = if cfg!(target_os = "windows") {
        &[".exe", ".msi"]
    } else if cfg!(target_os = "macos") {
        &[".dmg", ".zip"]
    } else {
        &[".AppImage", ".deb", ".rpm"]
    };

    for pat in patterns {
        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.to_ascii_lowercase().ends_with(&pat.to_ascii_lowercase()) {
                if let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                    return Some((name.to_string(), url.to_string()));
                }
            }
        }
    }
    None
}

/// start-update-download：检查 → 下载 → 打开安装包。
/// 立即返回 {success:true} 后台继续，进度经事件推送（对齐前端 AppUpdateDialog 预期）。
pub async fn start_update_download(app: AppHandle) -> Value {
    let current = app.package_info().version.to_string();

    let client = match reqwest::Client::builder()
        .user_agent("MoeKoeMusic-Updater")
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "message": e.to_string() }),
    };

    let release: Value = match client.get(RELEASE_API).send().await {
        Ok(resp) => match resp.json().await {
            Ok(v) => v,
            Err(e) => return json!({ "success": false, "message": format!("解析发布信息失败: {e}") }),
        },
        Err(e) => return json!({ "success": false, "message": format!("检查更新失败: {e}") }),
    };

    let latest = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if latest.is_empty() {
        return json!({ "success": false, "message": "无法获取最新版本信息" });
    }
    if !is_newer(&current, &latest) {
        return json!({ "success": false, "reason": "not-available" });
    }

    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let Some((name, url)) = pick_asset(&assets) else {
        // 没有当前平台的安装包 → 打开发布页
        if let Some(html) = release.get("html_url").and_then(|v| v.as_str()) {
            let _ = app.opener().open_url(html, None::<&str>);
        }
        return json!({ "success": false, "reason": "unsupported" });
    };

    // 后台下载
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = download_and_open(&app2, &client, &name, &url).await {
            let _ = app2.emit_to(
                crate::windows::MAIN,
                "update-error",
                json!({ "message": e }),
            );
        }
    });

    json!({ "success": true })
}

async fn download_and_open(
    app: &AppHandle,
    client: &reqwest::Client,
    name: &str,
    url: &str,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let dir = app
        .path()
        .temp_dir()
        .map_err(|e| e.to_string())?
        .join("moekoe-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_path = dir.join(name);
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut stream = resp.bytes_stream();
    let mut transferred: u64 = 0;
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载中断: {e}"))?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        transferred += chunk.len() as u64;

        if last_emit.elapsed().as_millis() >= 300 || transferred == total {
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let percent = if total > 0 {
                transferred as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let _ = app.emit_to(
                crate::windows::MAIN,
                "update-progress",
                json!({
                    "percent": percent,
                    "transferred": transferred,
                    "total": total,
                    "bytesPerSecond": (transferred as f64 / elapsed) as u64
                }),
            );
            last_emit = std::time::Instant::now();
        }
    }
    drop(file);

    let _ = app.emit_to(crate::windows::MAIN, "update-downloaded", ());

    // 打开安装包（Windows: 运行安装器；macOS: 挂载 dmg；Linux: 打开文件）
    let _ = app
        .opener()
        .open_path(file_path.to_string_lossy().to_string(), None::<&str>);
    Ok(())
}
