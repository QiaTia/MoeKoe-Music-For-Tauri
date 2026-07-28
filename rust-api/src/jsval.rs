//! JS 值 -> 字符串的两种语义，用于**精确复刻**签名拼接。
//! - `js_to_string`  : 对应模板字符串 `${value}` 的行为
//! - `js_json_stringify` : 对应 `JSON.stringify(value)`（compact，键序由 preserve_order 保证）

use serde_json::Value;

/// 对应 JS `${value}`
pub fn js_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(|x| match x {
                // JS Array.toString: null/undefined 元素变空串
                Value::Null => String::new(),
                other => js_to_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// 对应 JSON.stringify(value)（compact）
pub fn js_json_stringify(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// android 签名里单个 value 的取值：typeof === 'object' ? JSON.stringify : `${v}`
/// 注意 JS 中 typeof null / array 均为 'object'
pub fn sign_value_android(v: &Value) -> String {
    match v {
        Value::Object(_) | Value::Array(_) | Value::Null => js_json_stringify(v),
        _ => js_to_string(v),
    }
}
