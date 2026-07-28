//! 对应 util/util.js —— 通用工具。

use flate2::read::ZlibDecoder;
use num_bigint::BigUint;
use rand::Rng;
use std::io::Read;

use crate::crypto::crypto_md5;

const KEY_STRING: &[u8] = b"1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const NUM_STRING: &[u8] = b"1234567890";

/// 随机字符串（字符集含数字+大写字母，无小写）—— 对应 randomString
pub fn random_string(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let i = rng.gen_range(0..KEY_STRING.len());
            KEY_STRING[i] as char
        })
        .collect()
}

/// 纯数字随机串 —— 对应 randomNumber
pub fn random_number(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let i = rng.gen_range(0..NUM_STRING.len());
            NUM_STRING[i] as char
        })
        .collect()
}

/// 对应 parseCookieString：去掉 Domain/domain/path/expires=... 段 与 ;HttpOnly
pub fn parse_cookie_string(cookie: &str) -> String {
    // 原正则：/\s*(Domain|domain|path|expires)=[^(;|$)]+;*/g  ——去除属性段
    // 用简单的分号切分实现等价效果（保留 name=value，其余属性段丢弃）
    let mut parts: Vec<&str> = Vec::new();
    for seg in cookie.split(';') {
        let s = seg.trim();
        if s.is_empty() {
            continue;
        }
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("domain=")
            || lower.starts_with("path=")
            || lower.starts_with("expires=")
            || lower == "httponly"
        {
            continue;
        }
        parts.push(s);
    }
    parts.join("; ")
}

/// cookie 串 -> map（对应 cookieToJson）
pub fn cookie_to_json(cookie: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut obj = serde_json::Map::new();
    if cookie.is_empty() {
        return obj;
    }
    for pair in cookie.split(';') {
        let mut kv = pair.splitn(2, '=');
        // ⚠️ 与 Node 一致：不 trim key，前导空格会保留在 key 中（如 " userid"）
        let k = kv.next().unwrap_or("").to_string();
        let v = kv.next().map(|s| s.to_string());
        if let Some(v) = v {
            obj.insert(k, serde_json::Value::String(v));
        } else {
            // 与 JS 一致：无 '=' 时 value 为 undefined，这里存 null
            if !k.is_empty() {
                obj.insert(k, serde_json::Value::Null);
            }
        }
    }
    obj
}

/// KRC 歌词解码：base64 -> 跳过 4 字节魔数 -> 与 16 字节 key 循环 XOR -> zlib inflate -> utf8
pub fn decode_lyrics(val: &str) -> String {
    let bytes = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, val) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    if bytes.len() < 4 {
        return String::new();
    }
    let en_key: [u8; 16] = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];
    let mut krc: Vec<u8> = bytes[4..].to_vec();
    for (i, b) in krc.iter_mut().enumerate() {
        *b ^= en_key[i % en_key.len()];
    }
    let mut decoder = ZlibDecoder::new(&krc[..]);
    let mut out = String::new();
    match decoder.read_to_string(&mut out) {
        Ok(_) => out,
        Err(_) => String::new(),
    }
}

/// 对应 calculateMid：md5(str) 的 hex 当作 16 进制大整数，转十进制字符串
pub fn calculate_mid(str: &str) -> String {
    let digest = crypto_md5(str);
    match BigUint::parse_bytes(digest.as_bytes(), 16) {
        Some(n) => n.to_str_radix(10),
        None => "0".to_string(),
    }
}

/// 生成类 GUID 字符串（对应 getGuid）
pub fn get_guid() -> String {
    let mut rng = rand::thread_rng();
    let e = |rng: &mut rand::rngs::ThreadRng| -> String {
        let n: u32 = ((65536.0 * (1.0 + rng.gen::<f64>())) as u32) & 0xFFFF;
        // (x | 0).toString(16).substring(1) —— 取后 4 位 hex
        let s = format!("{:x}", 0x10000 | (n as u32 & 0xFFFF));
        s[1..].to_string()
    };
    format!(
        "{}{}-{}-{}-{}-{}{}{}",
        e(&mut rng),
        e(&mut rng),
        e(&mut rng),
        e(&mut rng),
        e(&mut rng),
        e(&mut rng),
        e(&mut rng),
        e(&mut rng)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_mid() {
        // Node 基准（parity_node.js）：calculateMid() 入参为空串
        // => md5("") = d41d8cd98f00b204e9800998ecf8427e
        // => 该 hex 作 16 进制大整数 -> 十进制
        let mid = calculate_mid("");
        assert_eq!(mid, "281949768489412648962353822266799178366");
    }

    #[test]
    fn test_cookie_to_json() {
        // 与 Node 一致：key 保留前导空格（" userid"）
        let m = cookie_to_json("token=abc; userid=123; vip_type=6");
        assert_eq!(m.get("token").unwrap(), "abc");
        assert_eq!(m.get(" userid").unwrap(), "123");
        assert_eq!(m.get(" vip_type").unwrap(), "6");
    }

    #[test]
    fn test_random_string_len() {
        assert_eq!(random_string(16).len(), 16);
        assert_eq!(random_number(6).len(), 6);
    }
}
