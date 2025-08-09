use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router,
};
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Declare our modules
mod api;
mod blockchain;
mod config;
mod mcp;
mod mcp_working;

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

    // Check if we should run as MCP server or HTTP server
    let args: Vec<String> = env::args().collect();
    let is_mcp_mode = args.len() > 1 && args[1] == "--mcp";

    if is_mcp_mode {
        // Run as MCP server
        tracing::info!("Starting as MCP server...");
        let mcp_server = mcp_working::McpServer::new(app_config);
        mcp_server.run().await.context("Failed to run MCP server")?;
    } else {
        // Run as HTTP server (original behavior)
        tracing::info!("Starting as HTTP server...");

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
            .route(
                "/fees/estimate/:chain_id", // Corrected path to match file
                post(api::fees::estimate_fees_handler),
            )
            .route("/health", get(api::health::health_handler))
            .route(
                "/transfer/:chain_id",
                post(api::transfer::transfer_sei_handler),
            )
            .with_state(app_config.clone());

        let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));

        tracing::info!("HTTP Server listening on {}", addr);

        axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
            .await
            .context("HTTP server failed")?;
    }

    Ok(())
}