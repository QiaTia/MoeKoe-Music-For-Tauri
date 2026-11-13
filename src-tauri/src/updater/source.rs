//! 更新源解析与 release 清单获取（含多源回退与结果缓存）。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::asset::{parse_assets, pick_from_assets, UpdateAsset};
use super::version::normalize_version;

/// 发布仓库（Tauri 版自身的 Releases，勿混用上游 Electron 仓库）
pub(super) const RELEASE_REPO: &str = "qiatia/MoeKoe-Music-For-Tauri";
/// 镜像前缀：受限网络下 API 与下载都经它转发
pub(super) const MIRROR_PREFIX: &str = "https://gh-proxy.com/";

/// 自定义源校验：只接受 https（本机回环允许 http，便于本地测试）。
///
/// Releases 清单决定「装哪个包、装哪个版本」，走 http 可被中间人替换成旧版本/恶意包；
/// 即便下载后做 sha256 比对，摘要同样来自该清单，所以传输层必须可信。
pub(super) fn normalize_endpoint(raw: &str) -> Result<String, String> {
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
pub(super) fn release_sources(custom: Option<&str>) -> Vec<(&'static str, String)> {
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
pub(super) fn download_urls(asset_url: &str) -> Vec<String> {
    let mut list = vec![asset_url.to_string()];
    if !asset_url.starts_with(MIRROR_PREFIX) {
        list.push(format!("{MIRROR_PREFIX}{asset_url}"));
    }
    list
}

pub(super) struct CheckedRelease {
    pub(super) version: String,
    pub(super) notes: String,
    pub(super) date: String,
    pub(super) html_url: String,
    pub(super) asset: Option<UpdateAsset>,
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

pub(super) async fn check_release(
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn check_cache_hits_and_respects_key() {
        use serde_json::json;
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

    /// 镜像冒烟：gh-proxy 必须能转发 API（受限网络下唯一的检查通道）
    #[tokio::test]
    #[ignore]
    async fn updater_mirror_smoke() {
        let client = crate::updater::download::build_client(None, Some(Duration::from_secs(20)))
            .expect("客户端初始化");
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

    /// 联网冒烟（默认忽略，CI 不依赖网络）：
    /// `cargo test --lib -- --ignored --nocapture updater_live_smoke`
    #[tokio::test]
    #[ignore]
    async fn updater_live_smoke() {
        use super::super::asset::{arch_rank, current_arch, current_platform};
        let client = crate::updater::download::build_client(
            None,
            Some(crate::updater::API_TIMEOUT),
        )
        .expect("客户端初始化");
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
}
