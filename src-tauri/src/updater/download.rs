//! HTTP 客户端、取消信号、下载与完整性校验。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

use super::asset::{safe_file_name, UpdateAsset};
use super::source::download_urls;
use super::{
    emit_progress, CANCELLED_MSG, CONNECT_TIMEOUT, DOWNLOAD_READ_TIMEOUT, PROGRESS_THROTTLE_MS,
};

/// 下载取消信号。
///
/// 用 `watch` 而不是 `AtomicBool` 轮询：轮询只能在一个分片到达后才能感知取消，
/// 弱网下 `stream.next()` 可能挂满 `read_timeout`（90s），用户点了「取消下载」
/// 却要等一分半才响应。`watch` 可同时被「读取当前态」和「唤醒等待中的循环」使用。
static CANCEL: OnceLock<(watch::Sender<bool>, watch::Receiver<bool>)> = OnceLock::new();

fn cancel_pair() -> &'static (watch::Sender<bool>, watch::Receiver<bool>) {
    CANCEL.get_or_init(|| watch::channel(false))
}

/// 复位取消状态。**必须在「开始新一轮下载」的入口同步调用**（早于下载任务启动），
/// 否则用户在检查阶段点了取消，会被下载开始时的复位吞掉。
pub(super) fn reset_cancel() {
    cancel_pair().0.send_replace(false);
}

pub(super) fn request_cancel() {
    cancel_pair().0.send_replace(true);
}

pub(super) fn cancel_requested() -> bool {
    *cancel_pair().1.borrow()
}

/// 交给下载循环的独立 receiver：可 `changed().await`，不会丢掉取消通知
fn cancel_receiver() -> watch::Receiver<bool> {
    cancel_pair().1.clone()
}

/// 判断一个错误是否代表「用户取消」（而不是真的失败）
pub(super) fn is_cancelled(msg: &str) -> bool {
    msg == CANCELLED_MSG
}

// ---------------- HTTP 客户端 ----------------

/// 从设置里取代理：仅 `proxy=on` 且 `proxyUrl` 非空时生效
pub(super) fn proxy_from_settings(store: &crate::settings::SettingsStore) -> Option<String> {
    if !store.setting_on("proxy") {
        return None;
    }
    let raw = store.setting_str("proxyUrl")?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    if raw.contains("://") {
        Some(raw)
    } else {
        Some(format!("http://{raw}"))
    }
}

pub(super) fn build_client(
    proxy: Option<&str>,
    total_timeout: Option<Duration>,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("MoeKoeMusic-Updater/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(DOWNLOAD_READ_TIMEOUT);
    // 检查阶段限制总耗时（8s）；下载阶段不限总耗时，只靠 read_timeout 兜底，
    // 否则 90MB 的 AppImage 会在 8 秒时被直接掐断。
    if let Some(t) = total_timeout {
        builder = builder.timeout(t);
    }
    builder = match proxy {
        Some(p) => {
            let parsed = reqwest::Proxy::all(p).map_err(|e| format!("代理地址无效: {e}"))?;
            builder.proxy(parsed)
        }
        // 显式 no_proxy：用户系统代理可能已失效（或指向本机某个已停止的端口），
        // 继承它会让更新请求直接失败，且错误信息极具误导性
        None => builder.no_proxy(),
    };
    builder
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {e}"))
}

// ---------------- 完整性校验 ----------------

fn file_sha256(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("读取安装包失败: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("读取安装包失败: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 校验 `sha256:<hex>` 形式的摘要
fn verify_sha256(path: &Path, digest: &str) -> Result<(), String> {
    let expected = digest
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(digest.trim())
        .to_ascii_lowercase();
    if expected.is_empty() {
        return Ok(());
    }
    let actual = file_sha256(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("安装包校验失败（期望 {expected}，实际 {actual}）"))
    }
}

/// 在阻塞线程池里算摘要。
///
/// 安装包最大近百 MB（AppImage 实测 89MB），同步读会让 tokio worker 卡住上百毫秒，
/// 期间该 worker 上的其它异步任务（WS、播放状态推送）全部停顿。
async fn verify_sha256_offloaded(path: PathBuf, digest: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || verify_sha256(&path, &digest))
        .await
        .map_err(|e| format!("校验任务异常: {e}"))?
}

/// 清理下载目录里的历史产物（旧版本安装包与残留 `.part`）。
///
/// 每次更新都按版本名落一个新文件、几十 MB 一个，不清理会随升级次数无限累积。
/// 只删本模块自己产生的后缀，避免误删同目录的其它内容；正在被安装器占用的文件
/// 删除会失败，忽略即可（下次更新再清）。
fn cleanup_old_downloads(dir: &Path, keep: &Path) {
    const INSTALLER_EXTS: &[&str] = &[".exe", ".msi", ".dmg", ".deb", ".rpm", ".appimage", ".tar.gz"];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep || !path.is_file() {
            continue;
        }
        let lower = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let owned = lower.ends_with(".part")
            || INSTALLER_EXTS.iter().any(|ext| lower.ends_with(ext));
        if owned {
            let _ = std::fs::remove_file(&path);
        }
    }
}

// ---------------- 下载 ----------------

/// 进度回调：(stage, 已下载, 总量, 字节/秒)
type ProgressFn<'a> = dyn Fn(&str, u64, u64, u64) + Send + Sync + 'a;

/// 下载单个 URL 到 `part_path`，边写边回调进度。
async fn fetch_to_file(
    client: &reqwest::Client,
    url: &str,
    part_path: &Path,
    expected_size: u64,
    progress: &ProgressFn<'_>,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(expected_size);

    let mut file = std::fs::File::create(part_path).map_err(|e| format!("创建临时文件失败: {e}"))?;
    let mut stream = resp.bytes_stream();
    let mut cancel = cancel_receiver();
    let mut transferred: u64 = 0;
    let started = Instant::now();
    let mut last_emit = Instant::now();

    // 进入循环前先看一次：用户可能在「检查阶段」或任务排队期间就点了取消，
    // 那种情况不会再产生 changed() 事件。临时文件由调用方统一清理。
    if cancel_requested() {
        return Err(CANCELLED_MSG.into());
    }

    loop {
        // biased：取消优先，避免取消的同时恰好有分片到达又写了一次盘
        tokio::select! {
            biased;
            _ = cancel.changed() => {
                if cancel_requested() {
                    // 此处 file 仍在作用域内，Windows 上删不掉已打开文件，
                    // 清理统一交给 download_asset 的 Err 分支
                    return Err(CANCELLED_MSG.into());
                }
            }
            item = stream.next() => {
                let Some(chunk) = item else { break };
                let chunk = chunk.map_err(|e| format!("下载中断: {e}"))?;
                file.write_all(&chunk).map_err(|e| format!("写入失败: {e}"))?;
                transferred += chunk.len() as u64;

                let finished = total > 0 && transferred >= total;
                if finished || last_emit.elapsed().as_millis() >= PROGRESS_THROTTLE_MS {
                    let elapsed = started.elapsed().as_secs_f64().max(0.001);
                    let bps = (transferred as f64 / elapsed) as u64;
                    progress("downloading", transferred, total, bps);
                    last_emit = Instant::now();
                }
            }
        }
    }
    file.flush().map_err(|e| format!("写入失败: {e}"))?;
    drop(file);

    if expected_size > 0 && transferred != expected_size {
        return Err(format!("下载不完整（{transferred}/{expected_size} 字节）"));
    }
    Ok(())
}

/// 下载到临时目录，逐个候选地址尝试；每个地址都做尺寸 + sha256 校验。
pub(super) async fn download_asset(
    app: &AppHandle,
    client: &reqwest::Client,
    asset: &UpdateAsset,
) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .temp_dir()
        .map_err(|e| e.to_string())?
        .join("moekoe-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建下载目录失败: {e}"))?;

    // 远端可控字段会参与本地路径拼接，先收紧成纯文件名（阻断 ../ 穿越）
    let file_name = safe_file_name(&asset.name)?;
    let final_path = dir.join(&file_name);
    let part_path = dir.join(format!("{}.part", file_name));
    let mut last_err = String::from("没有可用的下载地址");

    // 取消状态已在 start_update_download 的任务启动前复位；这里不能再复位，
    // 否则「检查阶段就点了取消」会被清掉，用户看着进度条继续下载下去。
    for (idx, url) in download_urls(&asset.url).into_iter().enumerate() {
        let emit = |stage: &str, done: u64, total: u64, bps: u64| {
            emit_progress(app, stage, done, total, bps)
        };
        match fetch_to_file(client, &url, &part_path, asset.size, &emit).await {
            Ok(()) => {
                if let Some(digest) = asset.digest.as_deref() {
                    // 校验要读完整包（最长数百毫秒），先把 UI 切到「正在校验」
                    emit_progress(app, "verifying", asset.size, asset.size, 0);
                    match verify_sha256_offloaded(part_path.clone(), digest.to_string()).await {
                        Ok(()) => {}
                        Err(e) => {
                            last_err = e;
                            let _ = std::fs::remove_file(&part_path);
                            continue; // 换下一个源（可能是镜像截断导致，不代表全部失败）
                        }
                    }
                }
                if final_path.exists() {
                    let _ = std::fs::remove_file(&final_path);
                }
                std::fs::rename(&part_path, &final_path)
                    .map_err(|e| format!("保存安装包失败: {e}"))?;
                // 清掉历史版本的安装包与残留分片，避免 temp 目录无限增长
                cleanup_old_downloads(&dir, &final_path);
                return Ok(final_path);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part_path);
                if is_cancelled(&e) {
                    return Err(e);
                }
                eprintln!("[updater] 下载地址 {} 失败: {e}", idx + 1);
                last_err = e;
            }
        }
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// 取消状态是全局的，涉及它的用例必须串行
    static CANCEL_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn cancel_signal_reset_and_request() {
        let _guard = CANCEL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // 全局状态：本用例自洽，不与其他用例交叉断言
        reset_cancel();
        assert!(!cancel_requested(), "复位后应为未取消");
        request_cancel();
        assert!(cancel_requested(), "请求取消后必须立即可见");
        reset_cancel();
        assert!(!cancel_requested(), "再次复位后应恢复");

        // 取消语义只认统一文案，避免各处手写字符串后失效
        assert!(is_cancelled(CANCELLED_MSG));
        assert!(!is_cancelled("下载中断: connection reset"));
    }

    /// 慢速流上点取消必须即时打断，而不是等下一个分片 / read_timeout（90s）。
    #[tokio::test]
    async fn cancel_interrupts_slow_stream() {
        use axum::{routing::get, Router};
        use futures_util::stream;
        use std::convert::Infallible;

        let _guard = CANCEL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // 每 100ms 才吐 1KB 且永不结束：轮询式取消在这里会卡到 read_timeout
        let app = Router::new().route(
            "/slow",
            get(|| async {
                let s = stream::unfold(0u32, |n| async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Some((Ok::<_, Infallible>(vec![0u8; 1024]), n + 1))
                });
                axum::body::Body::from_stream(s)
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        reset_cancel();
        let dir = std::env::temp_dir().join("moekoe-cancel-test");
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("slow.part");
        let _ = std::fs::remove_file(&part);

        let client = build_client(None, None).unwrap();
        let url = format!("http://{addr}/slow");
        let task = tokio::spawn(async move {
            let progress = |_: &str, _: u64, _: u64, _: u64| {};
            fetch_to_file(&client, &url, &part, 0, &progress).await
        });

        // 等它确实开始接收数据，再取消
        tokio::time::sleep(Duration::from_millis(350)).await;
        let started = Instant::now();
        request_cancel();

        let joined = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("取消必须打断等待中的数据流，不能等到 read_timeout")
            .expect("下载任务不应 panic");
        assert!(
            matches!(&joined, Err(e) if is_cancelled(e)),
            "应返回取消错误，实际: {joined:?}"
        );
        println!("[cancel] 打断耗时 {:?}", started.elapsed());

        let _ = std::fs::remove_file(&dir.join("slow.part"));
        reset_cancel();
    }

    #[test]
    fn sha256_verify_roundtrip() {
        let dir = std::env::temp_dir().join("moekoe-updater-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("payload.bin");
        std::fs::write(&file, b"hello").unwrap();
        // sha256("hello")
        let digest = "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&file, digest).is_ok());
        assert!(verify_sha256(&file, &digest.replace("2cf2", "0000")).is_err());
        // 无前缀也接受
        assert!(verify_sha256(&file, digest.strip_prefix("sha256:").unwrap()).is_ok());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn cleanup_removes_stale_installers_only() {
        let dir = std::env::temp_dir().join("moekoe-cleanup-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let keep = dir.join("MoeKoe.Music_1.7.2_x64-setup.exe");
        let part = dir.join("MoeKoe.Music_1.7.2_x64-setup.exe.part");
        let old = dir.join("MoeKoe.Music_1.7.1_x64-setup.exe");
        let foreign = dir.join("notes.txt");
        for p in [&keep, &part, &old, &foreign] {
            std::fs::write(p, b"x").unwrap();
        }

        cleanup_old_downloads(&dir, &keep);

        assert!(keep.exists(), "当前安装包必须保留");
        assert!(!part.exists(), "残留分片应被清理");
        assert!(!old.exists(), "旧版本安装包应被清理");
        assert!(foreign.exists(), "非本模块产生的文件不能动");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 下载冒烟（默认忽略）：经镜像真实下载当前平台安装包并校验 sha256，
    /// 验证「镜像 URL 拼接 + 流式落盘 + 摘要校验」整条链路。
    /// `cargo test --lib -- --ignored --nocapture updater_download_smoke`
    #[tokio::test]
    #[ignore]
    async fn updater_download_smoke() {
        use super::super::asset::{current_arch, current_platform};
        let check_client =
            build_client(None, Some(crate::updater::API_TIMEOUT)).expect("客户端初始化");
        let rel = super::super::source::check_release(
            &check_client,
            None,
            current_platform(),
            current_arch(),
        )
        .await
        .expect("应能取到 release 清单");
        let asset = rel.asset.expect("应能选到安装包");
        let digest = asset.digest.clone().expect("缺少摘要，无法校验");

        let client = build_client(None, None).expect("下载客户端初始化");
        let dir = std::env::temp_dir().join("moekoe-updater-smoke");
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join(format!("{}.part", asset.name));

        // 直连 github.com 在受限网络不可达，这里直接用镜像候选地址
        let urls = download_urls(&asset.url);
        let mirror = urls.last().expect("应有镜像地址").clone();
        let mut last_err = String::new();
        for url in [mirror, asset.url.clone()] {
            match fetch_to_file(&client, &url, &part, asset.size, &|_, _, _, _| {}).await {
                Ok(()) => {
                    if let Err(e) = verify_sha256(&part, &digest) {
                        last_err = e;
                        continue;
                    }
                    let size = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                    assert_eq!(size, asset.size, "落盘大小与清单不一致");
                    println!("[download] ok: {} bytes via {}", size, url);
                    let _ = std::fs::remove_file(&part);
                    return;
                }
                Err(e) => last_err = e,
            }
        }
        let _ = std::fs::remove_file(&part);
        panic!("所有下载地址均失败: {last_err}");
    }
}
