//! 对应 util/crypto.js —— 全部加密/摘要算法。
//!
//! 关键迁移事实（务必牢记）：
//! CryptoJS 把 key/iv 当作 **UTF-8 字节串** 直接使用（无 OpenSSL EvpKDF 派生、无 salt）。
//! 因此这里一律用 `key_str.as_bytes()` / `iv_str.as_bytes()` 直接喂入：
//!   - 32 字符 key -> 32 字节 -> AES-256
//!   - 16 字符 key -> 16 字节 -> AES-128

use aes::{Aes128, Aes192, Aes256};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use md5::{Digest as Md5Digest, Md5};
use num_bigint::BigUint;
use rsa::pkcs8::DecodePublicKey;
use rsa::traits::PublicKeyParts;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use sha1::Sha1;

use crate::util::random_string;

// 两对硬编码 RSA 公钥（正式版 / lite 版），与 Node 原样一致
pub const PUBLIC_RSA_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDIAG7QOELSYoIJvTFJhMpe1s/gbjDJX51HBNnEl5HXqTW6lQ7LC8jr9fWZTwusknp+sVGzwd40MwP6U5yDE27M/X1+UR4tvOGOqp94TJtQ1EPnWGWXngpeIW5GxoQGao1rmYWAu6oi1z9XkChrsUdC6DJE5E221wf/4WLFxwAtRQIDAQAB\n-----END PUBLIC KEY-----";
pub const PUBLIC_LITE_RSA_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDECi0Np2UR87scwrvTr72L6oO01rBbbBPriSDFPxr3Z5syug0O24QyQO8bg27+0+4kBzTBTBOZ/WWU0WryL1JSXRTXLgFVxtzIY41Pe7lPOgsfTCn5kZcvKhYKJesKnnJDNr5/abvTGf+rHG3YRwsCHcQ08/q6ifSioBszvb3QiwIDAQAB\n-----END PUBLIC KEY-----";

/// AES 加密返回值（对应 {str, key}）
#[derive(Debug, Clone)]
pub struct AesEncrypt {
    pub str: String,
    pub key: String,
}

// ---------------- 摘要 ----------------

/// MD5 -> hex（对象先 JSON.stringify，此处调用方自行 stringify 后传字符串）
pub fn crypto_md5(data: &str) -> String {
    let mut h = Md5::new();
    h.update(data.as_bytes());
    hex::encode(h.finalize())
}

/// SHA1 -> hex
pub fn crypto_sha1(data: &str) -> String {
    let mut h = Sha1::new();
    h.update(data.as_bytes());
    hex::encode(h.finalize())
}

// ---------------- AES-CBC/PKCS7 (key/iv 为字节串) ----------------

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    match key.len() {
        16 => cbc::Encryptor::<Aes128>::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(data),
        24 => cbc::Encryptor::<Aes192>::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(data),
        _ => cbc::Encryptor::<Aes256>::new_from_slices(key, iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(data),
    }
}

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    let res = match key.len() {
        16 => cbc::Decryptor::<Aes128>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(data),
        24 => cbc::Decryptor::<Aes192>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(data),
        _ => cbc::Decryptor::<Aes256>::new_from_slices(key, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(data),
    };
    res.unwrap_or_default()
}

/// 对应 cryptoAesEncrypt(data, opt)
///
/// ⚠️ 原样复刻源码 bug：`if (opt?.key && opt?.key) return hex;`
///   —— 只要显式传了 key（无论有没有 iv），返回值就是**裸 hex 字符串**（str=hex, key 无意义）。
///   本函数用 `explicit_key` 表达"是否显式传了 key"。
pub fn crypto_aes_encrypt(
    data: &str,
    opt_key: Option<&str>,
    opt_iv: Option<&str>,
) -> AesEncrypt {
    let (key, iv, temp_key) = if let (Some(k), Some(v)) = (opt_key, opt_iv) {
        (k.to_string(), v.to_string(), String::new())
    } else {
        let temp_key = opt_key
            .map(|s| s.to_string())
            .unwrap_or_else(|| random_string(16).to_lowercase());
        let key = crypto_md5(&temp_key)[..32].to_string();
        let iv = key[key.len() - 16..].to_string();
        (key, iv, temp_key)
    };

    let cipher = aes_cbc_encrypt(key.as_bytes(), iv.as_bytes(), data.as_bytes());
    let hex_str = hex::encode(cipher);

    AesEncrypt {
        str: hex_str,
        // 复刻 bug：显式 key 时 Node 返回裸 hex，此处 key 字段调用方应忽略
        key: temp_key,
    }
}

/// 对应 cryptoAesDecrypt(data_hex, key, iv?) -> 尝试 JSON.parse，失败返回原字符串
pub fn crypto_aes_decrypt(data_hex: &str, key: &str, iv: Option<&str>) -> serde_json::Value {
    let (real_key, real_iv) = if iv.is_none() {
        let k = crypto_md5(key)[..32].to_string();
        let v = k[k.len() - 16..].to_string();
        (k, v)
    } else {
        (key.to_string(), iv.unwrap().to_string())
    };

    let cipher = hex::decode(data_hex).unwrap_or_default();
    let plain = aes_cbc_decrypt(real_key.as_bytes(), real_iv.as_bytes(), &cipher);
    let text = String::from_utf8_lossy(&plain).to_string();
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => v,
        Err(_) => serde_json::Value::String(text),
    }
}

/// 歌单/云盘专用 AES-128：key=md5(rand6)[..16], iv=md5(rand6)[16..32]，Base64 输出
pub fn playlist_aes_encrypt(data: &str) -> AesEncrypt {
    let key = random_string(6).to_lowercase();
    let digest = crypto_md5(&key);
    let enc_key = &digest[..16];
    let iv = &digest[16..32];
    let cipher = aes_cbc_encrypt(enc_key.as_bytes(), iv.as_bytes(), data.as_bytes());
    AesEncrypt {
        str: B64.encode(cipher),
        key,
    }
}

/// 对应 playlistAesDecrypt({key, str(base64)})
pub fn playlist_aes_decrypt(key: &str, str_b64: &str) -> serde_json::Value {
    let digest = crypto_md5(key);
    let enc_key = &digest[..16];
    let iv = &digest[16..32];
    let cipher = B64.decode(str_b64).unwrap_or_default();
    let plain = aes_cbc_decrypt(enc_key.as_bytes(), iv.as_bytes(), &cipher);
    let text = String::from_utf8_lossy(&plain).to_string();
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => v,
        Err(_) => serde_json::Value::String(text),
    }
}

// ---------------- RSA ----------------

/// 解析 SPKI 公钥。
///
/// 注意：硬编码的 PEM 里 base64 是**单行**（与 Node forge 一致），
/// 而 `from_public_key_pem` 走 RFC7468 严格解码器要求 64 列换行，会报
/// `Base64(InvalidEncoding)`。因此这里手动剥离头尾+去空白+base64 解码，
/// 再走 DER 解析，规避行宽限制。
fn load_pub_key(pem: &str) -> RsaPublicKey {
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    let der = B64.decode(body.as_bytes()).expect("RSA 公钥 base64 解码失败");
    RsaPublicKey::from_public_key_der(&der).expect("RSA 公钥 DER 解析失败")
}

/// 对应 cryptoRSAEncrypt —— **原始无填充 RSA**：明文右补零到 keyLen(128B)，转大整数 modPow(e,n)，hex 输出。
pub fn crypto_rsa_encrypt(data: &str, public_key_pem: Option<&str>) -> String {
    let pem = public_key_pem.unwrap_or(if crate::config::is_lite() {
        PUBLIC_LITE_RSA_KEY
    } else {
        PUBLIC_RSA_KEY
    });
    let key = load_pub_key(pem);
    let n = BigUint::from_bytes_be(&key.n().to_bytes_be());
    let e = BigUint::from_bytes_be(&key.e().to_bytes_be());
    let key_len = (key.n().bits() + 7) / 8; // 向上取整字节数

    let buffer = data.as_bytes();
    assert!(buffer.len() <= key_len, "Data length exceeds key size");
    // padded = new Uint8Array(keyLength); padded.set(buffer)  -> 明文在前，尾部补零
    let mut padded = vec![0u8; key_len];
    padded[..buffer.len()].copy_from_slice(buffer);

    let m = BigUint::from_bytes_be(&padded);
    let c = m.modpow(&e, &n);
    let mut hexstr = c.to_str_radix(16);
    // 补齐到 keyLen*2 位
    let want = key_len * 2;
    if hexstr.len() < want {
        hexstr = format!("{}{}", "0".repeat(want - hexstr.len()), hexstr);
    }
    hexstr
}

/// 对应 rsaEncrypt2 —— RSAES-PKCS1-V1_5，hex 输出（随机填充，每次不同，与 node-forge 一致）
pub fn rsa_encrypt2(data: &str) -> String {
    let pem = if crate::config::is_lite() {
        PUBLIC_LITE_RSA_KEY
    } else {
        PUBLIC_RSA_KEY
    };
    let key = load_pub_key(pem);
    let mut rng = rand::thread_rng();
    let enc = key
        .encrypt(&mut rng, Pkcs1v15Encrypt, data.as_bytes())
        .expect("RSA PKCS1 加密失败");
    hex::encode(enc)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 与 Node crypto-js 对拍的固定向量（值取自 parity_node.js 实跑输出）
    #[test]
    fn test_md5() {
        assert_eq!(crypto_md5("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(crypto_md5(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(crypto_md5("hello"), "5d41402abc4b2a76b9719d911017c592");
        // md5(JSON.stringify({a:1,b:'x'})) == md5(r#"{"a":1,"b":"x"}"#)
        assert_eq!(crypto_md5(r#"{"a":1,"b":"x"}"#), "75a6ce4be04f5075316047b75a98d8eb");
    }

    #[test]
    fn test_sha1() {
        assert_eq!(crypto_sha1("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(crypto_sha1("hello"), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_aes_explicit_key_iv_256() {
        // key 32 字符 -> AES-256；与 Node cryptoAesEncrypt('hello world',{key,iv}) 对拍
        let key = "90b8382a1bb4ccdcf063102053fd75b8"; // 32 chars
        let iv = "f063102053fd75b8"; // 16 chars
        let out = crypto_aes_encrypt("hello world", Some(key), Some(iv));
        // Node bug 分支：opt.key 存在 -> 返回裸 hex
        assert_eq!(out.str, "a74cba63b174731fdda9156f4c521298");
        let back = crypto_aes_decrypt(&out.str, key, Some(iv));
        assert_eq!(back, serde_json::Value::String("hello world".into()));
    }

    #[test]
    fn test_aes_explicit_key_only_branch() {
        // opt.key 存在但无 iv：key 走 md5 派生，返回裸 hex（bug 分支）
        // Node: cryptoAesEncrypt('branch3',{key:'fixedkey'}) == 22f5359e07d80d7036c4af3178a725b1
        let out = crypto_aes_encrypt("branch3", Some("fixedkey"), None);
        assert_eq!(out.str, "22f5359e07d80d7036c4af3178a725b1");
        // 解密回环（解密时同样按 md5 派生，无需 iv）
        let back = crypto_aes_decrypt(&out.str, "fixedkey", None);
        assert_eq!(back, serde_json::Value::String("branch3".into()));
    }

    #[test]
    fn test_playlist_aes_roundtrip() {
        let enc = playlist_aes_encrypt("{\"a\":1}");
        let dec = playlist_aes_decrypt(&enc.key, &enc.str);
        assert_eq!(dec, serde_json::json!({"a":1}));
    }

    #[test]
    fn test_rsa_raw_vector() {
        // 原始无填充 RSA 是确定性的，直接对拍 Node 绝对值
        std::env::remove_var("platform"); // 非 lite -> 正式版公钥
        assert_eq!(
            crypto_rsa_encrypt("testdata123", None),
            "3ff863c39f0325a9a656c52cd44d02b9bb7ae8a9ae424215d78b3356b713036c2283283f94a0d90b881dfffc8754d2a3a4b72ebf4698cc73b5d571a019f95b4699652ff50fee9dca498a65e0b22a8b9b993259ca9846126787057a6033b1d4d3b62a0686c2eb9a36cedea741b4ee82e539230265ca4db8c83f30eaee2a879eff"
        );
    }
}
