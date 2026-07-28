//! 对应 util/runtime.js —— CLI 参数解析与环境覆盖。

use std::collections::HashMap;

/// 解析 `--key=value` 形式参数（对应 parseCliArgs）
pub fn parse_cli_args(args: &[String]) -> HashMap<String, String> {
    let mut acc = HashMap::new();
    for raw in args {
        let arg = raw.trim();
        if !arg.starts_with("--") {
            continue;
        }
        let eq = match arg.find('=') {
            Some(i) => i,
            None => continue,
        };
        // eqIndex <= 2 || eqIndex === len-1 -> 跳过
        if eq <= 2 || eq == arg.len() - 1 {
            continue;
        }
        let key = arg[2..eq].trim().to_string();
        let value = arg[eq + 1..].trim().to_string();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        acc.insert(key, value);
    }
    acc
}

/// 应用 CLI 覆盖到环境变量（对应 applyCliOverrides）
pub fn apply_cli_overrides(args: &[String]) {
    let parsed = parse_cli_args(args);
    if let Some(v) = parsed.get("proxy") {
        std::env::set_var("KUGOU_API_PROXY", v);
    }
    if let Some(v) = parsed.get("platform") {
        std::env::set_var("platform", v);
    }
    if let Some(v) = parsed.get("guid") {
        std::env::set_var("KUGOU_API_GUID", v);
    }
    if let Some(v) = parsed.get("dev") {
        std::env::set_var("KUGOU_API_DEV", v);
    }
    if let Some(v) = parsed.get("mac") {
        std::env::set_var("KUGOU_API_MAC", v);
    }
    if let Some(v) = parsed.get("port") {
        match v.parse::<i64>() {
            Ok(p) if p > 0 => std::env::set_var("PORT", p.to_string()),
            _ => eprintln!("[cli] Invalid port value \"{}\", fallback to default.", v),
        }
    }
}
