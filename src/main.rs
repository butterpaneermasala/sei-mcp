use anyhow::Result;
use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Declare our modules
mod api;
mod blockchain;
mod config;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sei_mcp_server_rs=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();
    let app_config = config::AppConfig::from_env()?;

    let app = Router::new()
        .route(
            "/balance/:chain_id/:address",
            get(api::balance::get_balance_handler),
        )
        .route("/wallet/create", post(api::wallet::create_wallet_handler))
        .route("/wallet/import", post(api::wallet::import_wallet_handler))
        .route(
            "/history/:chain_id/:address",
            get(api::history::get_transaction_history_handler),
        )
        .route("/fees/estimate", post(api::fees::estimate_fees_handler))
        .with_state(app_config.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));

    tracing::info!("Server listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}
