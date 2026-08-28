//! 对应 util/helper.js —— 全部签名算法。
//! 全部读取 env platform 实时判定 lite（与 Node helper.js 一致）。

use serde_json::{Map, Value};

use crate::config::{is_lite, use_appid, use_clientver};
use crate::crypto::crypto_md5;
use crate::jsval::{js_to_string, sign_value_android};
use md5::{Digest as Md5Digest, Md5};
use hex;

/// 签名加密类型（对应 createRequest 的 options.encryptType）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncryptType {
    #[default]
    Android,
    Web,
    Register,
}

/// web 版 signature：`map(k=>"k=v").sort().join('')` —— **先拼 k=v 再排序字符串**
pub fn signature_web(params: &Map<String, Value>) -> String {
    let s = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
    let mut kvs: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, js_to_string(v)))
        .collect();
    kvs.sort();
    let ps = kvs.join("");
    crypto_md5(&format!("{}{}{}", s, ps, s))
}

/// web 版带请求体 signature（对应 KuGouMusicApi 组队模块内联的 `signatureWebParamsWithBody`）：
/// `sort(keys).map(k=>"k=v").join('')` + bodyStr，盐值同 web 版（NVPh5oo...），body 夹中间。
/// 与 `signature_web` 的区别：① 先排序 key 再拼（而非先拼 k=v 再排序字符串）；② 夹入 bodyStr。
/// 用于组队活动 create_team / join_team 两个 POST 端点。
pub fn signature_web_with_body(params: &Map<String, Value>, body: &str) -> String {
    let s = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let ps: String = keys
        .iter()
        .map(|k| format!("{}={}", k, js_to_string(&params[*k])))
        .collect::<Vec<_>>()
        .join("");
    crypto_md5(&format!("{}{}{}{}", s, ps, body, s))
}

/// android 版 signature：`sort(keys).map(k=>"k="+val).join('')` —— **先排序 key 再拼**，data 夹中间
pub fn signature_android(params: &Map<String, Value>, data: &str) -> String {
    let s = if is_lite() {
        "LnT6xpN3khm36zse0QzvmgTZ3waWdRSA"
    } else {
        "OIlwieks28dk2k092lksi2UIkp"
    };
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let ps: String = keys
        .iter()
        .map(|k| format!("{}={}", k, sign_value_android(&params[*k])))
        .collect::<Vec<_>>()
        .join("");
    crypto_md5(&format!("{}{}{}{}", s, ps, data, s))
}

/// android 版签名（二进制 body 场景）：`secret + sorted(k=v) + data_bytes + secret` 的流式 MD5。
///
/// 对应 KuGouMusicApi util/helper.js 的 `signatureAndroidParams` 二进制分支：
/// `MD5(str + paramsString + Buffer + str)`，`str` 固定为非 lite 密钥 `OIlwieks28dk2k092lksi2UIkp`。
/// 与普通 android 签名不同，此处密钥**不随 platform=lite 切换**，与参考实现保持一致。
pub fn signature_android_binary(params: &Map<String, Value>, data: &[u8]) -> String {
    let s = "OIlwieks28dk2k092lksi2UIkp";
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let ps: String = keys
        .iter()
        .map(|k| format!("{}={}", k, sign_value_android(&params[*k])))
        .collect::<Vec<_>>()
        .join("");
    let mut h = Md5::new();
    h.update(s.as_bytes());
    h.update(ps.as_bytes());
    h.update(data);
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// register 版 signature：只对 value 排序（死代码，仍实现以备用）
pub fn signature_register(params: &Map<String, Value>) -> String {
    let mut vals: Vec<String> = params.values().map(js_to_string).collect();
    vals.sort();
    let ps = vals.join("");
    crypto_md5(&format!("1014{}1014", ps))
}

/// sign：`sort(keys).map(k=>k+v).join('')` + data + secret
pub fn sign_params(params: &Map<String, Value>, data: &str) -> String {
    let s = "R6snCXJgbCaj9WFRJKefTMIFp0ey6Gza";
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let ps: String = keys
        .iter()
        .map(|k| format!("{}{}", k, js_to_string(&params[*k])))
        .collect::<Vec<_>>()
        .join("");
    crypto_md5(&format!("{}{}{}", ps, data, s))
}

/// 音频直链 key：md5(hash + secret + appid + mid + userid)
pub fn sign_key(hash: &str, mid: &str, userid: &str, appid: &str) -> String {
    let s = if is_lite() {
        "185672dd44712f60bb1736df5a377e82"
    } else {
        "57ae12eb6890223e355ccfcb74edf70d"
    };
    let appid = if appid.is_empty() {
        use_appid().to_string()
    } else {
        appid.to_string()
    };
    let userid = if userid.is_empty() { "0".to_string() } else { userid.to_string() };
    crypto_md5(&format!("{}{}{}{}{}", hash, s, appid, mid, userid))
}

/// 云盘 key：md5("musicclound" + hash + pid + secret)
pub fn sign_cloud_key(hash: &str, pid: &str) -> String {
    let s = "ebd1ac3134c880bda6a2194537843caa0162e2e7";
    crypto_md5(&format!("musicclound{}{}{}", hash, pid, s))
}

/// signParamsKey：md5(appid + secret + clientver + data)
pub fn sign_params_key(data: &str, appid: Option<i64>, clientver: Option<i64>) -> String {
    let s = if is_lite() {
        "LnT6xpN3khm36zse0QzvmgTZ3waWdRSA"
    } else {
        "OIlwieks28dk2k092lksi2UIkp"
    };
    let appid = appid.unwrap_or_else(use_appid);
    let clientver = clientver.unwrap_or_else(use_clientver);
    crypto_md5(&format!("{}{}{}{}", appid, s, clientver, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn test_signature_android_vector() {
        // 手工按算法核算： secret + sorted("k=v") + data + secret 的 md5
        let params = map(json!({"b": 2, "a": 1}));
        let sig = signature_android(&params, "");
        // sorted keys: a,b -> "a=1b=2"
        let expect = crypto_md5(&format!(
            "{}{}{}{}",
            "OIlwieks28dk2k092lksi2UIkp", "a=1b=2", "", "OIlwieks28dk2k092lksi2UIkp"
        ));
        assert_eq!(sig, expect);
    }

    #[test]
    fn test_signature_web_vector() {
        let params = map(json!({"b": 2, "a": 1}));
        let sig = signature_web(&params);
        // map->["b=2","a=1"] sort-> ["a=1","b=2"] join "a=1b=2"
        let expect = crypto_md5(&format!(
            "{}{}{}",
            "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt", "a=1b=2", "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt"
        ));
        assert_eq!(sig, expect);
    }

    #[test]
    fn test_signature_web_with_body_vector() {
        // 与 Node signatureWebParamsWithBody 对拍的绝对值：
        // sort(keys) -> "a=1b=2"，salt + ps + "BODY" + salt 的 md5。
        let params = map(json!({"b": 2, "a": 1}));
        assert_eq!(
            signature_web_with_body(&params, "BODY"),
            "0e64c8443f569e85fb6f405fe8be2798"
        );
        // 与无 body 的 signature_web 结果必须不同（body 参与了签名）
        assert_ne!(
            signature_web_with_body(&params, "BODY"),
            signature_web(&params)
        );
    }

    #[test]
    fn test_signature_android_with_object_value() {
        // 对象值走 JSON.stringify
        let params = map(json!({"data": {"x": 1}}));
        let sig = signature_android(&params, "");
        let expect = crypto_md5(&format!(
            "{}{}{}{}",
            "OIlwieks28dk2k092lksi2UIkp", "data={\"x\":1}", "", "OIlwieks28dk2k092lksi2UIkp"
        ));
        assert_eq!(sig, expect);
    }

    /// 与 Node parity_node.js 逐一硬对拍的绝对值向量。
    /// 固定输入 paramsA 覆盖 number/string/中文/object/array/null/bool，
    /// 精确验证 android(JSON.stringify) 与 web(`${}`) 两套强制转换逻辑。
    #[test]
    fn test_parity_vectors_vs_node() {
        // 确保非 lite 分支（与 Node 默认 env 一致）
        std::env::remove_var("platform");
        let params = map(json!({
            "appid": 1005, "clientver": 20489, "clienttime": 1700000000i64,
            "mid": "abc", "keyword": "周杰伦", "obj": {"x": 1},
            "arr": [1, 2], "nul": null, "bool": true
        }));

        assert_eq!(signature_android(&params, ""), "8a480f0872f50a68918dc5dfe89ad7c5");
        assert_eq!(signature_web(&params), "fb8652f3f0e51ccb460661d06aebaa42");
        assert_eq!(sign_params(&params, "{\"d\":1}"), "2c1fe52d95932840268a564c476e739a");
        assert_eq!(sign_params_key("{\"a\":1}", Some(1005), Some(20489)), "bf5f982e382fa9be227a74178f391374");
        assert_eq!(sign_key("somehash", "somemid", "12345", "1005"), "f897496d8ccc3447bed0be260a2ffbe0");
        assert_eq!(sign_cloud_key("HASH123", "999"), "d17f8a01a864c6ecc35c703237c48fc8");
    }
}
