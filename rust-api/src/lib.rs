//! kugoumusicapi 库入口。
//!
//! 两种使用方式：
//! 1. bin (main.rs)：独立 HTTP 服务（axum），行为与 Node 版完全对齐。
//! 2. lib：供 Tauri 等宿主进程内直调 —— 走 `dispatch::ApiDispatcher`，
//!    跳过 HTTP 编解码，直接进入模块分发，语义与 server.rs 保持一致。

pub mod cli;
pub mod config;
pub mod crypto;
pub mod dispatch;
pub mod jsval;
pub mod modules;
pub mod request;
pub mod server;
pub mod sign;
pub mod util;
