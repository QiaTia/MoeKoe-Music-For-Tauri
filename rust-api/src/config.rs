//! 对应 util/config.json + util/index.js 的平台判定。
//! 关键：Node 里 helper.js / request.js 在**每次调用时**读取 process.env.platform，
//! 因此这里所有 is_lite()/use_appid() 也都实时读取环境变量，保持行为一致。

use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub wx_appid: String,
    pub wx_lite_appid: String,
    pub wx_secret: String,
    pub wx_lite_secret: String,
    pub srcappid: i64,
    pub appid: i64,
    pub apiver: i64,
    pub clientver: i64,
    #[serde(rename = "liteAppid")]
    pub lite_appid: i64,
    #[serde(rename = "liteClientver")]
    pub lite_clientver: i64,
}

pub static CONFIG: Lazy<Config> =
    Lazy::new(|| serde_json::from_str(include_str!("../config.json")).expect("config.json 解析失败"));

/// process.env.platform === 'lite'
pub fn is_lite() -> bool {
    std::env::var("platform").map(|v| v == "lite").unwrap_or(false)
}

/// 当前生效的 appid（手机版 / lite）
pub fn use_appid() -> i64 {
    if is_lite() {
        CONFIG.lite_appid
    } else {
        CONFIG.appid
    }
}

/// 当前生效的 clientver（手机版 / lite）
pub fn use_clientver() -> i64 {
    if is_lite() {
        CONFIG.lite_clientver
    } else {
        CONFIG.clientver
    }
}
