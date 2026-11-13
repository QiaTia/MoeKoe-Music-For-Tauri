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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::watch;

/// 发布仓库（Tauri 版自身的 Releases，勿混用上游 Electron 仓库）
const RELEASE_REPO: &str = "qiatia/MoeKoe-Music-For-Tauri";
/// 镜像前缀：受限网络下 API 与下载都经它转发
const MIRROR_PREFIX: &str = "https://gh-proxy.com/";
/// 检查阶段的整体超时（单个源）
const API_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 下载阶段：两次读数据之间的最长等待，避免大包慢速网络被整体超时打断
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(90);
/// 进度事件节流（分片回调极密集，200ms 一次足够平滑且不会造成 IPC 洪水）
const PROGRESS_THROTTLE_MS: u128 = 200;

/// 下载取消信号。
///
/// 用 `watch` 而不是 `AtomicBool` 轮询：轮询只能在一个分片到达后才能感知取消，
/// 弱网下 `stream.next()` 可能挂满 `read_timeout`（90s），用户点了「取消下载」
/// 却要等一分半才响应。`watch` 可同时被「读取当前态」和「唤醒等待中的循环」使用。
static CANCEL: OnceLock<(watch::Sender<bool>, watch::Receiver<bool>)> = OnceLock::new();
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

fn cancel_pair() -> &'static (watch::Sender<bool>, watch::Receiver<bool>) {
    CANCEL.get_or_init(|| watch::channel(false))
}

/// 复位取消状态。**必须在「开始新一轮下载」的入口同步调用**（早于下载任务启动），
/// 否则用户在检查阶段点了取消，会被下载开始时的复位吞掉。
fn reset_cancel() {
    cancel_pair().0.send_replace(false);
}

fn request_cancel() {
    cancel_pair().0.send_replace(true);
}

fn cancel_requested() -> bool {
    *cancel_pair().1.borrow()
}

/// 交给下载循环的独立 receiver：可 `changed().await`，不会丢掉取消通知
fn cancel_receiver() -> watch::Receiver<bool> {
    cancel_pair().1.clone()
}

/// 判断一个错误是否代表「用户取消」（而不是真的失败）
fn is_cancelled(msg: &str) -> bool {
    msg == CANCELLED_MSG
}

fn last_installer() -> &'static Mutex<Option<PathBuf>> {
    LAST_INSTALLER.get_or_init(|| Mutex::new(None))
}

// ---------------- 版本比较 ----------------

fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
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

/// 去掉 tag 前缀 `v`，前端展示与版本比对都用这个形态
fn normalize_version(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

// ---------------- 更新源 ----------------

/// 自定义源校验：只接受 https（本机回环允许 http，便于本地测试）。
///
/// Releases 清单决定「装哪个包、装哪个版本」，走 http 可被中间人替换成旧版本/恶意包；
/// 即便下载后做 sha256 比对，摘要同样来自该清单，所以传输层必须可信。
fn normalize_endpoint(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("更新源地址为空".into());
    }
    let url = url::Url::parse(raw).map_err(|e| format!("更新源地址非法: {e}"))?;
    match url.scheme() {
        "https" => Ok(url.to_string()),
        "http" if matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")) => {
            Ok(url.to_string())
        }
        _ => Err("更新源必须是 HTTPS（仅本机回环地址允许 http）".into()),
    }
}

/// 候选源：自定义源 → GitHub 直连 → gh-proxy 镜像。
fn release_sources(custom: Option<&str>) -> Vec<(&'static str, String)> {
    let official = format!("https://api.github.com/repos/{RELEASE_REPO}/releases/latest");
    let mirror = format!("{MIRROR_PREFIX}{official}");
    let mut list: Vec<(&'static str, String)> = Vec::new();

    if let Some(raw) = custom.map(str::trim).filter(|s| !s.is_empty()) {
        match normalize_endpoint(raw) {
            Ok(url) if url != official && url != mirror => list.push(("custom", url)),
            Ok(_) => {}
            Err(e) => eprintln!("[updater] 自定义更新源无效，忽略并回退官方源: {e}"),
        }
    }
    list.push(("github", official));
    list.push(("mirror", mirror));
    list
}

/// 下载候选地址：原始 github.com 地址 + 镜像前缀地址。
///
/// 已经是镜像地址的不再二次加前缀。
fn download_urls(asset_url: &str) -> Vec<String> {
    let mut list = vec![asset_url.to_string()];
    if !asset_url.starts_with(MIRROR_PREFIX) {
        list.push(format!("{MIRROR_PREFIX}{asset_url}"));
    }
    list
}

// ---------------- 资产选择 ----------------

#[derive(Debug, Clone, PartialEq)]
struct UpdateAsset {
    name: String,
    url: String,
    size: u64,
    /// GitHub API 的 `digest` 字段，形如 `sha256:...`；缺失时为 None
    digest: Option<String>,
}

/// 把远端可控的资产名收紧成安全的本地文件名。
///
/// `assets[].name` 由发布方决定（自定义更新源 / `MOEKOE_UPDATE_SOURCE` 时更是完全外部可控），
/// 直接 `join` 到目录上会允许 `../../xxx` 逃出临时目录写到任意路径。安装包名不可能
/// 含路径分隔符或 Windows 保留字符，因此这里只接受纯文件名。
fn safe_file_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(format!("更新包文件名非法: {raw:?}"));
    }
    if name.len() > 128 {
        return Err(format!("更新包文件名过长（{} 字符）", name.len()));
    }
    let illegal = name
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'));
    if illegal || name.contains("..") {
        return Err(format!("更新包文件名非法: {raw:?}"));
    }
    Ok(name.to_string())
}

fn parse_assets(raw: &[Value]) -> Vec<UpdateAsset> {
    raw.iter()
        .filter_map(|a| {
            let raw_name = a.get("name").and_then(|v| v.as_str())?;
            // 非法名在选包阶段就丢弃，避免带着穿越路径一路走到写盘
            let name = match safe_file_name(raw_name) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("[updater] 忽略非法资产: {e}");
                    return None;
                }
            };
            let url = a.get("browser_download_url").and_then(|v| v.as_str())?.to_string();
            Some(UpdateAsset {
                name,
                url,
                size: a.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                digest: a
                    .get("digest")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
            })
        })
        .collect()
}

/// 各平台接受的安装包后缀，按优先级排列
fn ext_priority(platform: &str) -> &'static [&'static str] {
    match platform {
        "windows" => &[".exe", ".msi"],
        // 只取 .dmg：`.app.tar.gz` 是 Tauri 的 updater artifact，open_path 只会解压，
        // 用户拿到的不是安装动作。缺失 dmg 时宁可提示「手动更新」也不下发不可用的包。
        "macos" => &[".dmg"],
        _ => &[".appimage", ".deb", ".rpm"],
    }
}

/// 文件名与当前架构的匹配度：`Some(2)` 明确匹配，`Some(1)` 未标注架构（可用），`None` 明确不匹配。
fn arch_rank(name: &str, arch: &str) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    let is_arm = n.contains("aarch64") || n.contains("arm64");
    let is_x64 = n.contains("x64") || n.contains("amd64") || n.contains("x86_64");
    match (is_arm, is_x64) {
        (true, true) => Some(1), // 罕见：两种都出现，交给顺序决定
        (true, false) => {
            if arch == "aarch64" {
                Some(2)
            } else {
                None
            }
        }
        (false, true) => {
            if arch == "aarch64" {
                None
            } else {
                Some(2)
            }
        }
        (false, false) => Some(1),
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// 选包：先按后缀优先级，再按架构匹配度；明确架构不匹配的直接排除。
fn pick_from_assets(
    assets: &[UpdateAsset],
    platform: &str,
    arch: &str,
) -> Option<UpdateAsset> {
    for ext in ext_priority(platform) {
        let mut best: Option<(u8, &UpdateAsset)> = None;
        for asset in assets {
            if !asset.name.to_ascii_lowercase().ends_with(ext) {
                continue;
            }
            let Some(rank) = arch_rank(&asset.name, arch) else {
                continue;
            };
            if best.map(|(r, _)| rank > r).unwrap_or(true) {
                best = Some((rank, asset));
            }
        }
        if let Some((_, asset)) = best {
            return Some(asset.clone());
        }
    }
    None
}

// ---------------- HTTP 客户端 ----------------

/// 从设置里取代理：仅 `proxy=on` 且 `proxyUrl` 非空时生效
fn proxy_from_settings(store: &crate::settings::SettingsStore) -> Option<String> {
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

fn build_client(
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

// ---------------- 检查 ----------------

struct CheckedRelease {
    version: String,
    notes: String,
    date: String,
    html_url: String,
    asset: Option<UpdateAsset>,
}

/// 检查结果缓存。
///
/// GitHub 匿名 API 只有 60 次/小时，而更新弹窗在 Header 与 SidebarNavigation 里各挂了一份，
/// 反复切换视图或重启客户端很容易把配额烧光。缓存按「更新源」隔离，5 分钟内复用。
const CHECK_CACHE_TTL: Duration = Duration::from_secs(300);
static CHECK_CACHE: OnceLock<Mutex<Option<(String, Instant, Value)>>> = OnceLock::new();

fn cache_slot() -> &'static Mutex<Option<(String, Instant, Value)>> {
    CHECK_CACHE.get_or_init(|| Mutex::new(None))
}

fn cache_key(custom: Option<&str>) -> String {
    custom
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("__official__")
        .to_string()
}

fn cache_get(key: &str) -> Option<Value> {
    let guard = cache_slot().lock().ok()?;
    let (k, at, value) = guard.as_ref()?;
    (k == key && at.elapsed() < CHECK_CACHE_TTL).then(|| value.clone())
}

fn cache_put(key: &str, value: &Value) {
    if let Ok(mut guard) = cache_slot().lock() {
        *guard = Some((key.to_string(), Instant::now(), value.clone()));
    }
}

/// 逐个源请求 latest release，返回首个成功的解析结果。
async fn fetch_latest(
    client: &reqwest::Client,
    custom: Option<&str>,
) -> Result<Value, String> {
    let key = cache_key(custom);
    if let Some(cached) = cache_get(&key) {
        return Ok(cached);
    }

    let mut last_err = String::from("没有可用的更新源");
    for (label, url) in release_sources(custom) {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                Ok(v) => {
                    let tag = v.get("tag_name").and_then(|x| x.as_str()).unwrap_or("");
                    if tag.is_empty() {
                        last_err = format!("[{label}] 发布信息缺少 tag_name");
                        continue;
                    }
                    cache_put(&key, &v);
                    return Ok(v);
                }
                Err(e) => last_err = format!("[{label}] 解析发布信息失败: {e}"),
            },
            Ok(resp) => {
                let status = resp.status();
                // 403 / 429 基本都是匿名 API 限流：镜像转发的是同一个接口、共用同一配额，
                // 继续换源也拿不到结果，直接把状态码抛给用户更是毫无意义
                last_err = if matches!(status.as_u16(), 403 | 429) {
                    format!("更新服务请求过于频繁（HTTP {status}），请稍后再试")
                } else {
                    format!("[{label}] HTTP {status}")
                };
            }
            Err(e) => last_err = format!("[{label}] {e}"),
        }
        eprintln!("[updater] 更新源不可用: {last_err}");
    }
    Err(last_err)
}

async fn check_release(
    client: &reqwest::Client,
    custom: Option<&str>,
    platform: &str,
    arch: &str,
) -> Result<CheckedRelease, String> {
    let release = fetch_latest(client, custom).await?;
    let raw_assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let assets = parse_assets(&raw_assets);
    Ok(CheckedRelease {
        version: normalize_version(
            release.get("tag_name").and_then(|v| v.as_str()).unwrap_or(""),
        ),
        notes: release
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        date: release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        html_url: release
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        asset: pick_from_assets(&assets, platform, arch),
    })
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

// ---------------- 下载 ----------------

fn emit_progress(app: &AppHandle, stage: &str, transferred: u64, total: u64, bps: u64) {
    let percent = if total > 0 {
        (transferred as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
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
            "bytesPerSecond": bps,
            "stage": stage,
        }),
    );
}

fn emit_error(app: &AppHandle, message: &str, cancelled: bool) {
    let _ = app.emit_to(
        crate::windows::MAIN,
        "update-error",
        json!({ "message": message, "cancelled": cancelled }),
    );
}

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

    if let Some(expected) = (expected_size > 0).then_some(expected_size) {
        if transferred != expected {
            return Err(format!("下载不完整（{transferred}/{expected} 字节）"));
        }
    }
    Ok(())
}

/// 下载到临时目录，逐个候选地址尝试；每个地址都做尺寸 + sha256 校验。
async fn download_asset(
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

// ---------------- 安装 ----------------

fn launch_installer(app: &AppHandle, path: &Path) -> Result<(), String> {
    // Linux 的 AppImage 下载后没有可执行位，直接 open 会被当成文档
    #[cfg(unix)]
    {
        let is_appimage = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("appimage"))
            .unwrap_or(false);
        if is_appimage {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(path).map_err(|e| format!("读取安装包失败: {e}"))?;
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)
                .map_err(|e| format!("设置可执行权限失败: {e}"))?;
        }
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("无法打开安装包: {e}"))
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

/// 从可执行文件向上回溯，找到所属的 `.app` bundle。
///
/// 找不到必须返回 `None`。早期实现用 `loop` 一路回溯到根目录，`tauri dev` 或
/// 未打包运行时会把它当成 bundle，生成 `sudo xattr -cr '/'` —— 用户照着粘贴
/// 等于递归清除整盘的隔离属性。
///
/// 仅 macOS 分支与单测需要它，其它平台的正式构建不编译，避免 unused 警告。
#[cfg(any(target_os = "macos", test))]
fn find_app_bundle(exe: &Path) -> Option<&Path> {
    exe.ancestors()
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("app"))
}

/// macOS Gatekeeper 放行脚本：清除 .app 的隔离属性。
///
/// 未签名/未公证的应用经自动更新替换后，首次打开可能被拦截（「已损坏」）。
/// 路径从当前可执行文件回溯到 `.app` bundle，安装到非默认位置也准确。
/// 非 macOS、或当前不是 `.app` 形态时返回空串（前端按空值不展示）。
#[tauri::command]
pub fn update_gatekeeper_script() -> String {
    #[cfg(target_os = "macos")]
    {
        let Ok(exe) = std::env::current_exe() else {
            return String::new();
        };
        let Some(bundle) = find_app_bundle(&exe) else {
            eprintln!(
                "[updater] 未找到 .app bundle，跳过 Gatekeeper 建议: {}",
                exe.display()
            );
            return String::new();
        };
        // 该命令由用户手动粘贴执行：路径要按 shell 字面量处理，单引号内不做任何展开
        let quoted = format!("'{}'", bundle.to_string_lossy().replace('\'', "'\\''"));
        format!("sudo xattr -cr {quoted}")
    }
    #[cfg(not(target_os = "macos"))]
    {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, size: u64) -> UpdateAsset {
        UpdateAsset {
            name: name.into(),
            url: format!("https://github.com/o/r/releases/download/v1/{name}"),
            size,
            digest: Some("sha256:abc".into()),
        }
    }

    #[test]
    fn version_compare() {
        assert!(is_newer("1.7.1", "1.7.2"));
        assert!(is_newer("1.7.1", "v1.8.0"));
        assert!(is_newer("1.9.0", "1.10.0"), "1.10 必须大于 1.9");
        assert!(!is_newer("1.7.2", "1.7.2"));
        assert!(!is_newer("1.8.0", "1.7.9"));
        assert!(is_newer("1.7.1", "1.7.1-1"), "同版本带尾号视为更新");
    }

    #[test]
    fn version_normalize() {
        assert_eq!(normalize_version("v1.7.2"), "1.7.2");
        assert_eq!(normalize_version(" 1.7.2 "), "1.7.2");
    }

    #[test]
    fn pick_windows_asset_by_arch() {
        let assets = vec![
            asset("MoeKoe.Music_1.7.1_arm64-setup.exe", 6),
            asset("MoeKoe.Music_1.7.1_x64-setup.exe", 7),
            asset("MoeKoe.Music_1.7.1_x64_en-US.msi", 9),
        ];
        assert_eq!(
            pick_from_assets(&assets, "windows", "x64").unwrap().name,
            "MoeKoe.Music_1.7.1_x64-setup.exe"
        );
        assert_eq!(
            pick_from_assets(&assets, "windows", "aarch64").unwrap().name,
            "MoeKoe.Music_1.7.1_arm64-setup.exe"
        );
    }

    #[test]
    fn pick_macos_prefers_matching_arch() {
        let assets = vec![
            asset("MoeKoe.Music_1.7.1_x64.dmg", 11),
            asset("MoeKoe.Music_1.7.1_aarch64.dmg", 11),
        ];
        assert_eq!(
            pick_from_assets(&assets, "macos", "aarch64").unwrap().name,
            "MoeKoe.Music_1.7.1_aarch64.dmg"
        );
        assert_eq!(
            pick_from_assets(&assets, "macos", "x64").unwrap().name,
            "MoeKoe.Music_1.7.1_x64.dmg"
        );
    }

    #[test]
    fn pick_linux_prefers_appimage_over_deb() {
        let assets = vec![
            asset("MoeKoe.Music_1.7.1_amd64.deb", 11),
            asset("MoeKoe.Music_1.7.1_amd64.AppImage", 89),
        ];
        assert_eq!(
            pick_from_assets(&assets, "linux", "x64").unwrap().name,
            "MoeKoe.Music_1.7.1_amd64.AppImage"
        );
    }

    #[test]
    fn pick_returns_none_when_only_wrong_arch() {
        let assets = vec![asset("MoeKoe.Music_1.7.1_arm64-setup.exe", 6)];
        assert!(pick_from_assets(&assets, "windows", "x64").is_none());
    }

    #[test]
    fn pick_accepts_unlabeled_arch() {
        let assets = vec![asset("MoeKoe.Music_1.7.1_setup.exe", 6)];
        assert!(pick_from_assets(&assets, "windows", "x64").is_some());
    }

    #[test]
    fn sources_order_and_dedup() {
        let list = release_sources(None);
        assert_eq!(list[0].0, "github");
        assert_eq!(list[1].0, "mirror");
        assert!(list[1].1.starts_with(MIRROR_PREFIX));

        // 自定义源排在最前
        let list = release_sources(Some("https://example.com/latest.json"));
        assert_eq!(list[0].0, "custom");
        assert_eq!(list.len(), 3);

        // 与官方源相同则不重复
        let list = release_sources(Some(&format!(
            "https://api.github.com/repos/{RELEASE_REPO}/releases/latest"
        )));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn endpoint_requires_https_except_loopback() {
        assert!(normalize_endpoint("https://example.com/a.json").is_ok());
        assert!(normalize_endpoint("http://127.0.0.1:8080/a.json").is_ok());
        assert!(normalize_endpoint("http://example.com/a.json").is_err());
        assert!(normalize_endpoint("ftp://example.com/a.json").is_err());
        assert!(normalize_endpoint("   ").is_err());
    }

    #[test]
    fn download_urls_adds_mirror_once() {
        let urls = download_urls("https://github.com/o/r/releases/download/v1/a.exe");
        assert_eq!(urls.len(), 2);
        assert!(urls[1].starts_with(MIRROR_PREFIX));

        let already = format!("{MIRROR_PREFIX}https://github.com/o/r/a.exe");
        assert_eq!(download_urls(&already).len(), 1);
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
    fn find_app_bundle_requires_dot_app() {
        // 正常打包形态：向上回溯到 .app
        let exe = Path::new("/Applications/MoeKoe Music.app/Contents/MacOS/moekoe-music");
        assert_eq!(
            find_app_bundle(exe),
            Some(Path::new("/Applications/MoeKoe Music.app"))
        );
        // 开发运行 / 裸二进制：必须返回 None，绝不能回落到根目录
        // （否则生成的命令是 `sudo xattr -cr '/'`）
        assert_eq!(find_app_bundle(Path::new("/usr/local/bin/moekoe-music")), None);
        assert_eq!(find_app_bundle(Path::new("/moekoe-music")), None);
    }

    #[test]
    fn safe_file_name_rejects_traversal() {
        let too_long = format!("{}.exe", "a".repeat(200));
        let mut bad_names = vec![
            "../../evil.exe",
            "..\\..\\evil.exe",
            "sub/dir/a.exe",
            "sub\\a.exe",
            "..",
            ".",
            "",
            "   ",
            "a\u{0}b.exe",
            "C:evil.exe",
            "a.exe:stream",
        ];
        bad_names.push(too_long.as_str());
        for bad in bad_names {
            assert!(safe_file_name(bad).is_err(), "应拒绝非法文件名: {bad:?}");
        }

        // 真实发布资产名必须全部放行
        for ok in [
            "MoeKoe.Music_1.7.1_x64-setup.exe",
            "MoeKoe.Music_aarch64.app.tar.gz",
            "MoeKoe.Music_1.7.1_amd64.AppImage",
            "MoeKoe.Music_1.7.1_x64.dmg",
        ] {
            assert_eq!(safe_file_name(ok).unwrap(), ok);
        }
    }

    #[test]
    fn parse_assets_drops_traversal_names() {
        let raw = json!([
            { "name": "../../evil.exe", "browser_download_url": "https://x/e.exe", "size": 1 },
            { "name": "ok.exe", "browser_download_url": "https://x/ok.exe", "size": 2 }
        ]);
        let parsed = parse_assets(raw.as_array().unwrap());
        assert_eq!(parsed.len(), 1, "带路径穿越的资产必须被丢弃");
        assert_eq!(parsed[0].name, "ok.exe");
    }

    #[test]
    fn macos_asset_skips_updater_artifact() {
        // `.app.tar.gz` 是 Tauri 的 updater artifact，双击只会解压，不能当安装包
        let macos = ext_priority("macos");
        assert!(macos.contains(&".dmg"));
        assert!(!macos.contains(&".app.tar.gz"), "不能把 tar.gz 当安装包下发");

        let only_tar = vec![asset("MoeKoe.Music_aarch64.app.tar.gz", 9)];
        assert!(
            pick_from_assets(&only_tar, "macos", "aarch64").is_none(),
            "只剩 .app.tar.gz 时应判为无可用安装包（让用户手动更新）"
        );
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

    #[test]
    fn check_cache_hits_and_respects_key() {
        let key = "unit-test-source";
        cache_put(key, &json!({ "tag_name": "v9.9.9" }));
        assert_eq!(
            cache_get(key)
                .and_then(|v| v.get("tag_name").and_then(|x| x.as_str()).map(str::to_string)),
            Some("v9.9.9".to_string()),
            "同源应命中缓存"
        );
        assert!(cache_get("another-source").is_none(), "不同更新源不共享缓存");

        // 缓存是全局的，清掉避免影响其它用例
        if let Ok(mut guard) = cache_slot().lock() {
            *guard = None;
        }
    }

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

    /// 联网冒烟（默认忽略，CI 不依赖网络）：
    /// `cargo test --lib -- --ignored --nocapture updater_live_smoke`
    #[tokio::test]
    #[ignore]
    async fn updater_live_smoke() {
        let client = build_client(None, Some(API_TIMEOUT)).expect("客户端初始化");
        let rel = check_release(&client, None, current_platform(), current_arch())
            .await
            .expect("应能取到 release 清单");
        assert!(!rel.version.is_empty(), "版本号为空");
        let asset = rel.asset.expect("当前平台应能选到安装包");
        assert!(asset.size > 0, "安装包大小为 0");
        assert!(
            asset.digest.as_deref().unwrap_or("").starts_with("sha256:"),
            "缺少 sha256 摘要，无法校验完整性: {:?}",
            asset.digest
        );
        assert!(
            arch_rank(&asset.name, current_arch()).map(|r| r > 0).unwrap_or(false),
            "选到的包与当前架构不匹配: {}",
            asset.name
        );
        println!(
            "[live] version={} asset={} size={} digest={:?}",
            rel.version, asset.name, asset.size, asset.digest
        );
    }

    /// 下载冒烟（默认忽略）：经镜像真实下载当前平台安装包并校验 sha256，
    /// 验证「镜像 URL 拼接 + 流式落盘 + 摘要校验」整条链路。
    /// `cargo test --lib -- --ignored --nocapture updater_download_smoke`
    #[tokio::test]
    #[ignore]
    async fn updater_download_smoke() {
        let check_client = build_client(None, Some(API_TIMEOUT)).expect("客户端初始化");
        let rel = check_release(&check_client, None, current_platform(), current_arch())
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

    /// 镜像冒烟：gh-proxy 必须能转发 API（受限网络下唯一的检查通道）
    #[tokio::test]
    #[ignore]
    async fn updater_mirror_smoke() {
        let client = build_client(None, Some(Duration::from_secs(20))).expect("客户端初始化");
        let list = release_sources(None);
        let mirror = list
            .iter()
            .find(|(label, _)| *label == "mirror")
            .map(|(_, url)| url.clone())
            .expect("应存在镜像源");
        let resp = client.get(&mirror).send().await.expect("镜像请求失败");
        assert!(resp.status().is_success(), "镜像返回 {}", resp.status());
        let v: Value = resp.json().await.expect("镜像响应不是 JSON");
        assert!(v.get("tag_name").and_then(|x| x.as_str()).is_some());
        println!("[mirror] ok: {}", mirror);
    }

    #[test]
    fn parse_assets_reads_digest_and_size() {
        let raw = json!([{
            "name": "a.exe",
            "browser_download_url": "https://github.com/o/r/a.exe",
            "size": 123,
            "digest": "sha256:deadbeef"
        }]);
        let parsed = parse_assets(raw.as_array().unwrap());
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].size, 123);
        assert_eq!(parsed[0].digest.as_deref(), Some("sha256:deadbeef"));
    }
}
