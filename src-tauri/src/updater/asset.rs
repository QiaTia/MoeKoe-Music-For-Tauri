//! 安装包资产：解析远端清单、收紧文件名、按平台与架构选包。

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct UpdateAsset {
    pub(super) name: String,
    pub(super) url: String,
    pub(super) size: u64,
    /// GitHub API 的 `digest` 字段，形如 `sha256:...`；缺失时为 None
    pub(super) digest: Option<String>,
}

/// 把远端可控的资产名收紧成安全的本地文件名。
///
/// `assets[].name` 由发布方决定（自定义更新源 / `MOEKOE_UPDATE_SOURCE` 时更是完全外部可控），
/// 直接 `join` 到目录上会允许 `../../xxx` 逃出临时目录写到任意路径。安装包名不可能
/// 含路径分隔符或 Windows 保留字符，因此这里只接受纯文件名。
pub(super) fn safe_file_name(raw: &str) -> Result<String, String> {
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

pub(super) fn parse_assets(raw: &[Value]) -> Vec<UpdateAsset> {
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
pub(super) fn ext_priority(platform: &str) -> &'static [&'static str] {
    match platform {
        "windows" => &[".exe", ".msi"],
        // 只取 .dmg：`.app.tar.gz` 是 Tauri 的 updater artifact，open_path 只会解压，
        // 用户拿到的不是安装动作。缺失 dmg 时宁可提示「手动更新」也不下发不可用的包。
        "macos" => &[".dmg"],
        _ => &[".appimage", ".deb", ".rpm"],
    }
}

/// 文件名与当前架构的匹配度：`Some(2)` 明确匹配，`Some(1)` 未标注架构（可用），`None` 明确不匹配。
pub(super) fn arch_rank(name: &str, arch: &str) -> Option<u8> {
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

pub(super) fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

pub(super) fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// 选包：先按后缀优先级，再按架构匹配度；明确架构不匹配的直接排除。
pub(super) fn pick_from_assets(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn asset(name: &str, size: u64) -> UpdateAsset {
        UpdateAsset {
            name: name.into(),
            url: format!("https://github.com/o/r/releases/download/v1/{name}"),
            size,
            digest: Some("sha256:abc".into()),
        }
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
