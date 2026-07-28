use kugoumusicapi::{cli, server};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // .env 加载（对应 dotenv.config）
    let _ = dotenvy::dotenv();

    // CLI 覆盖（对应 applyCliOverrides）
    let args: Vec<String> = std::env::args().skip(1).collect();
    cli::apply_cli_overrides(&args);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    let state = Arc::new(server::AppState::new());
    let app = server::build_router(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("无法绑定 {}: {}", addr, e));
    println!("server running @ http://{}:{}", if host == "0.0.0.0" { "localhost" } else { &host }, port);

    axum::serve(listener, app).await.expect("服务异常退出");
}
