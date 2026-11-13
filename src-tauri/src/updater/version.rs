//! 版本号比较与规范化。
//!
//! release tag 固定为 `vX.Y.Z` 形态，这里只做「按数字段逐位比较」，不实现完整 semver。

/// 拆成数字段：`v1.7.2` → `[1, 7, 2]`，非数字段（如 `beta`）直接忽略。
pub(super) fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u64>().ok())
        .collect()
}

/// `latest` 是否比 `current` 新（缺省位按 0 处理，因此 1.10 > 1.9）。
pub(super) fn is_newer(current: &str, latest: &str) -> bool {
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
pub(super) fn normalize_version(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
