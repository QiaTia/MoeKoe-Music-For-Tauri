//! 在线更新：多源检查 → 完整性校验 → 下载（进度 / 可取消）→ 拉起安装包。
//!
//! 参考 Watt-Toolkit 的自动更新实现，但沿用本项目既有的「GitHub Releases + reqwest」
//! 方案：`tauri-plugin-updater` 要求 `createUpdaterArtifacts` + `TAURI_SIGNING_PRIVATE_KEY`
//! 的签名链路，本项目 CI 尚未配置密钥，直接换插件会让所有更新在验签阶段失败。
//!
//! 关键设计：
//! 1. **多源顺序回退**：GitHub API 直连在部分网络不可达，而 `endpoints` 数组式回退
//!    只对非 2xx 生效、对超时/连接重置无效；这里逐个源单独请求并设短超时，失败再试下一个。
//! 2. **下载地址同样回退**：asset 的 `browser_download_url` 指向 `github.com`，
//!    受限网络需要加 gh-proxy 前缀才能下载。
//! 3. **完整性校验**：GitHub API 的 `assets[].digest` 给出 `sha256:...`，
//!    下载后必须比对，校验不通过绝不打开安装包。
//! 4. **按架构选包**：Windows / macOS 同时发布 x64 与 arm64 安装包，
//!    错拿会让 Apple Silicon 跑 x64 包（或反之）。
//! 5. **走应用自身代理设置**：`proxy=on` 时用 `proxyUrl`，否则显式 `no_proxy`，
//!    避免继承到已失效的系统代理。
//! 6. **版本一致性**：检查与下载之间若发布了新版本，要求重新检查，
//!    避免静默装到与弹窗展示不同的版本。
//!
//! 事件契约（与前端 AppUpdateDialog 保持一致，勿改名）：
//! - `update-progress`：`{ percent, transferred, total, bytesPerSecond, stage }`
//! - `update-downloaded`：`{ path, version }`
//! - `update-error`：`{ message, cancelled? }`
//!
//! 模块划分：
//! - `version`：版本号比较
//! - `asset`：清单解析、文件名收紧、按平台/架构选包
//! - `source`：更新源、多源回退、检查结果缓存
//! - `download`：HTTP 客户端、取消信号、下载与 sha256 校验
//! - `install`：拉起安装包、Gatekeeper 辅助
//! - 本文件：全局状态（下载互斥 / 取消 / 最近安装包）、事件发送、对外命令

mod asset;
mod download;
mod install;
mod source;
mod version;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use asset::{current_arch, current_platform};
use download::{
    build_client, cancel_requested, download_asset, is_cancelled, proxy_from_settings,
    request_cancel, reset_cancel,
};
use install::launch_installer;
use source::check_release;
use version::{is_newer, normalize_version};

/// 检查阶段的整体超时（单个源）
const API_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 下载阶段：两次读数据之间的最长等待，避免大包慢速网络被整体超时打断
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(90);
/// 进度事件节流（分片回调极密集，200ms 一次足够平滑且不会造成 IPC 洪水）
const PROGRESS_THROTTLE_MS: u128 = 200;
/// 取消行为的统一文案；下游只允许通过 `is_cancelled()` 判定，避免字符串各处漂移
const CANCELLED_MSG: &str = "已取消更新";

/// 最近一次下载完成的安装包路径（供「重新安装」使用）
static LAST_INSTALLER: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// 下载互斥标记。
///
/// IPC 命令可被重复调用（更新弹窗在 Header 与 SidebarNavigation 各挂一份，用户也可能连点），
/// 两个任务并发写同一个 `.part` 会互相截断，`rename` 也会竞态。
static DOWNLOADING: AtomicBool = AtomicBool::new(false);

/// 占用标记的 RAII 归位：下载任务无论正常结束、失败还是 panic 都会释放
struct DownloadGuard;

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        DOWNLOADING.store(false, Ordering::SeqCst);
    }
}

/// 提前返回时释放占用标记（`DownloadGuard` 只能覆盖下载任务本身）
fn release_downloading(payload: Value) -> Value {
    DOWNLOADING.store(false, Ordering::SeqCst);
    payload
}

fn last_installer() -> &'static Mutex<Option<PathBuf>> {
    LAST_INSTALLER.get_or_init(|| Mutex::new(None))
}

// ---------------- 事件 ----------------

fn emit_progress(app: &AppHandle, stage: &str, transferred: u64, total: u64, bps: u64) {
    let percent = if total > 0 {
        (transferred as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    // 进度事件高频，发送失败（主窗口已销毁）保持静默
    let _ = app.emit_to(
        crate::windows::MAIN,
        "update-progress",
        json!({
            "percent": percent,
            "transferred": transferred,
            "total": total,
            "bytesPerSecond": bps,
            "stage": stage,
        }),
    );
}

fn emit_error(app: &AppHandle, message: &str, cancelled: bool) {
    // 主窗口可能已销毁（用户正在退出），发送失败只记日志，不影响下载流程收尾
    if let Err(e) = app.emit_to(
        crate::windows::MAIN,
        "update-error",
        json!({ "message": message, "cancelled": cancelled }),
    ) {
        eprintln!("[updater] 错误事件发送失败: {e}（原始错误: {message}）");
    }
}

// ---------------- 命令 ----------------

/// 检查更新（不下载）。前端「新版本」徽标与更新弹窗都走这里，
/// 保证与下载使用同一份 release 清单（此前前端直连上游仓库，会与后端版本不一致）。
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Value {
    let current = app.package_info().version.to_string();
    let ctx = app.state::<crate::AppCtx>();
    let proxy = proxy_from_settings(&ctx.settings);
    let client = match build_client(proxy.as_deref(), Some(API_TIMEOUT)) {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "message": e }),
    };

    let custom = custom_source(&ctx);
    match check_release(&client, custom.as_deref(), current_platform(), current_arch()).await {
        Ok(rel) => {
            let available = is_newer(&current, &rel.version);
            let asset = rel.asset.as_ref();
            json!({
                "success": true,
                "available": available,
                "currentVersion": current,
                "latestVersion": rel.version,
                "notes": rel.notes,
                "publishedAt": rel.date,
                "htmlUrl": rel.html_url,
                "assetName": asset.map(|a| a.name.clone()),
                "assetSize": asset.map(|a| a.size).unwrap_or(0),
                "digest": asset.and_then(|a| a.digest.clone()),
                "viaProxy": proxy.is_some(),
            })
        }
        Err(e) => json!({ "success": false, "message": e }),
    }
}

/// 开始下载并在完成后拉起安装包。
///
/// `expected_version` 用于「检查 → 下载」之间的一致性校验：期间若发布更高版本，
/// 返回错误让用户重新检查，而不是静默装成另一个版本。
pub async fn start_update_download(app: AppHandle, expected_version: Option<String>) -> Value {
    // 先占位、后复位取消：若已有任务在跑，此时复位会打断「正在进行中的取消」，
    // 所以必须先拒绝，再清理上一轮遗留的取消状态。
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return json!({
            "success": false,
            "reason": "busy",
            "message": "已有更新正在下载，请等待完成或先取消本次下载",
        });
    }
    // 复位点必须在命令入口（同步执行，早于任何下载任务）。若放在下载任务内部，
    // 用户在「检查更新」阶段点的取消会被随后的复位吞掉。
    reset_cancel();

    let current = app.package_info().version.to_string();
    let ctx = app.state::<crate::AppCtx>();
    let proxy = proxy_from_settings(&ctx.settings);
    let custom = custom_source(&ctx);

    let client = match build_client(proxy.as_deref(), Some(API_TIMEOUT)) {
        Ok(c) => c,
        Err(e) => return release_downloading(json!({ "success": false, "message": e })),
    };

    let rel = match check_release(&client, custom.as_deref(), current_platform(), current_arch()).await
    {
        Ok(r) => r,
        Err(e) => {
            return release_downloading(
                json!({ "success": false, "message": format!("检查更新失败: {e}") }),
            )
        }
    };

    if !is_newer(&current, &rel.version) {
        return release_downloading(
            json!({ "success": false, "reason": "not-available", "latestVersion": rel.version }),
        );
    }
    if let Some(expected) = expected_version.as_deref().map(normalize_version) {
        if !expected.is_empty() && expected != rel.version {
            return release_downloading(json!({
                "success": false,
                "reason": "version-changed",
                "message": format!("更新版本已变化（{expected} → {}），请重新检查更新", rel.version),
                "latestVersion": rel.version,
            }));
        }
    }

    let Some(asset) = rel.asset else {
        // 当前平台没有对应安装包 → 打开发布页让用户手动选
        if !rel.html_url.is_empty() {
            let _ = app.opener().open_url(rel.html_url.clone(), None::<&str>);
        }
        return release_downloading(
            json!({ "success": false, "reason": "unsupported", "latestVersion": rel.version }),
        );
    };

    let version = rel.version.clone();
    let asset_name = asset.name.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        // 占用标记随任务结束自动释放
        let _guard = DownloadGuard;
        // 用户在「检查更新」阶段就点了取消：不必再发一次下载请求
        if cancel_requested() {
            emit_error(&app2, CANCELLED_MSG, true);
            return;
        }
        // 下载用独立客户端：读取超时更宽松，避免大包慢速网络被中断
        let dl_client = match build_client(proxy_for_download(&app2).as_deref(), None) {
            Ok(c) => c,
            Err(e) => {
                emit_error(&app2, &e, false);
                return;
            }
        };
        match download_asset(&app2, &dl_client, &asset).await {
            Ok(path) => {
                if let Ok(mut slot) = last_installer().lock() {
                    *slot = Some(path.clone());
                }
                let _ = app2.emit_to(
                    crate::windows::MAIN,
                    "update-downloaded",
                    json!({ "path": path.to_string_lossy(), "version": version }),
                );
                if let Err(e) = launch_installer(&app2, &path) {
                    emit_error(&app2, &e, false);
                }
            }
            Err(e) => {
                let cancelled = is_cancelled(&e);
                emit_error(&app2, &e, cancelled);
            }
        }
    });

    json!({ "success": true, "latestVersion": rel.version, "assetName": asset_name })
}

/// 自定义更新源：优先读设置项；环境变量仅在显式开启时才生效。
///
/// 环境变量可以让客户端去下载并自动拉起任意安装包的安装程序（sha256 只能证明
/// 「与该源的清单一致」，清单本身也来自该源），因此必须由使用者明确开启，
/// 不能让任何能设置进程环境的程序单方面改写更新源。
fn custom_source(ctx: &crate::AppCtx) -> Option<String> {
    if let Some(v) = ctx
        .settings
        .setting_str("updateSource")
        .filter(|s| !s.trim().is_empty())
    {
        return Some(v);
    }
    let env_allowed = std::env::var("MOEKOE_ALLOW_UPDATE_SOURCE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !env_allowed {
        return None;
    }
    std::env::var("MOEKOE_UPDATE_SOURCE")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// 下载线程里重新读一次代理设置（设置可能在检查后被修改）
fn proxy_for_download(app: &AppHandle) -> Option<String> {
    let ctx = app.state::<crate::AppCtx>();
    proxy_from_settings(&ctx.settings)
}

/// 取消进行中的下载
#[tauri::command]
pub fn cancel_update_download() -> Value {
    request_cancel();
    json!({ "success": true })
}

/// 重新拉起已下载的安装包（用户关掉安装器后可再次打开）
#[tauri::command]
pub fn install_update(app: AppHandle) -> Value {
    let path = last_installer()
        .lock()
        .ok()
        .and_then(|g| g.clone());
    let Some(path) = path.filter(|p| p.exists()) else {
        return json!({ "success": false, "message": "更新包不存在，请重新下载" });
    };
    match launch_installer(&app, &path) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "message": e }),
    }
}

/// macOS 的 Gatekeeper 放行命令（其它平台返回空串）
#[tauri::command]
pub fn update_gatekeeper_script() -> String {
    install::gatekeeper_script()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_flag_is_exclusive_and_released_by_guard() {
        DOWNLOADING.store(false, Ordering::SeqCst);
        assert!(!DOWNLOADING.swap(true, Ordering::SeqCst), "首次占位应成功");
        assert!(DOWNLOADING.swap(true, Ordering::SeqCst), "占用中必须拒绝第二次");
        {
            let _guard = DownloadGuard;
        }
        assert!(
            !DOWNLOADING.load(Ordering::SeqCst),
            "guard 释放后应允许再次下载"
        );
    }
}
