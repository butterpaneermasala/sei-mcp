use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router,
};
use std::{env, io, net::SocketAddr, sync::Once};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Declare our modules
mod api;
mod blockchain;
mod config;
mod mcp;
mod mcp_working;

static TRACING_INIT: Once = Once::new();

fn init_tracing(is_mcp_mode: bool) {
    TRACING_INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sei_mcp_server_rs=debug,tower_http=debug".into());

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(io::stderr)
            .with_ansi(!is_mcp_mode); // Disable ANSI colors in MCP mode

        let result = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init();

        if result.is_err() {
            // This will now only be called if there is a legitimate error
            // during initialization, not because a logger is already set.
            eprintln!("Failed to initialize tracing subscriber");
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    // Check if we should run as MCP server or HTTP server
    let args: Vec<String> = env::args().collect();
    let is_mcp_mode = args.iter().any(|arg| arg == "--mcp");

    // Setup logging
    init_tracing(is_mcp_mode);

    // Load .env file
    dotenvy::dotenv().ok();
    let app_config = config::AppConfig::from_env()?;

    if is_mcp_mode {
        // Run as MCP server
        tracing::info!("Starting as MCP server...");
        let mcp_server = mcp_working::McpServer::new(app_config);
        mcp_server.run().await.context("Failed to run MCP server")?;
    } else {
        // Run as HTTP server
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
                "/fees/estimate/:chain_id",
                post(api::fees::estimate_fees_handler),
            )
            .route("/health", get(api::health::health_handler))
            .route("/search-events", get(api::event::search_events))
            .route("/get-contract-events", get(api::event::get_contract_events))
            .route(
                "/subscribe-contract-events",
                get(api::event::subscribe_contract_events),
            )
            .route(
                "/transfer/:chain_id",
                post(api::transfer::transfer_sei_handler),
            )
            .with_state(app_config.clone());

        let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));

        tracing::info!("HTTP Server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app)
            .await
            .context("HTTP server failed")?;
    }

    Ok(())
}
