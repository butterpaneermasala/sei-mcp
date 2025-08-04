// src/main.rs

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use anyhow::Result;

// Declare our modules
mod api;
mod config;
mod blockchain;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for structured logging.
    // It reads the `RUST_LOG` environment variable for filtering (e.g., RUST_LOG=info,debug).
    // If not set, it defaults to "sei_mcp_server_rs=debug,tower_http=debug".
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sei_mcp_server_rs=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load application configuration from environment variables (including .env file).
    // `dotenvy::dotenv().ok()` loads the .env file, ignoring errors if it doesn't exist.
    dotenvy::dotenv().ok();
    let app_config = config::AppConfig::from_env()?;

    // Build our Axum application with a single route for balance queries.
    // The `with_state` method allows us to share the `AppConfig` across handlers.
    // FIX: Changed `:address` to `{address}` for path parameter syntax.
    let app = Router::new()
        .route("/balance/{address}", get(api::balance::get_balance_handler))
        .with_state(app_config.clone()); // `AppConfig` must implement `Clone`

    // Set the server address to listen on all available network interfaces (0.0.0.0)
    // at the port specified in the configuration.
    let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));

    // Log that the server is starting.
    tracing::info!("Server listening on {}", addr);

    // Start the Axum server.
    // `tokio::net::TcpListener::bind(addr).await?` creates a TCP listener.
    // `axum::serve` then takes this listener and our application router to serve requests.
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await?;

    Ok(())
}
