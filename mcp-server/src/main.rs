// src/main.rs

use axum::{routing::get, routing::post, Router};
use sei_mcp_server_rs::AppState;
use sei_mcp_server_rs::{
    api::{
        balance::get_balance_handler,
        faucet::request_faucet,
        health::health_handler,
        history::get_transaction_history_handler,
        contract::{get_contract_handler, get_contract_code_handler, get_contract_transactions_handler},
        tx::send_transaction_handler,
        wallet::{create_wallet_handler, import_wallet_handler},
        discord::post_discord_handler,
        docs::redirect_to_seidocs_handler,
        seistream::{
            get_chain_info_handler,
            get_transaction_info_handler,
            get_transaction_history_handler as sei_get_transaction_history_handler,
            get_nft_metadata_items_handler,
        },
    },
    blockchain::client::SeiClient,
    blockchain::nonce_manager::NonceManager,
    config::Config,
    mcp::wallet_storage::{get_wallet_storage_path, WalletStorage},
    mcp::{
        handler::handle_mcp_request,
        protocol::{error_codes, Request, Response},
    },
};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
// removed HandleErrorLayer-based mapping; ConcurrencyLimit is not used
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
// Removed rpassword import - no longer needed for startup

// --- HTTP Server Logic ---
async fn run_http_server(state: AppState) {
    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/wallet/create", post(create_wallet_handler))
        .route("/api/wallet/import", post(import_wallet_handler))
        .route("/api/balance/:chain_id/:address", get(get_balance_handler))
        .route(
            "/api/history/:chain_id/:address",
            get(get_transaction_history_handler),
        )
        // Contract inspection routes
        .route("/contract/:chain_id/:address", get(get_contract_handler))
        .route(
            "/contract/:chain_id/:address/code",
            get(get_contract_code_handler),
        )
        .route(
            "/contract/:chain_id/:address/transactions",
            get(get_contract_transactions_handler),
        )
        // Discord integration route
        .route("/api/discord/post", post(post_discord_handler))
        // Redirect to Sei docs
        .route("/redirect/seidocs", get(redirect_to_seidocs_handler))
        .route("/api/faucet/request", post(request_faucet))
        .route("/api/tx/send", post(send_transaction_handler))
        // SeiStream mirror endpoints
        .route("/api/chain/network", get(get_chain_info_handler))
        .route("/api/transactions/evm/:hash", get(get_transaction_info_handler))
        .route(
            "/api/accounts/evm/:address/transactions",
            get(sei_get_transaction_history_handler),
        )
        .route(
            "/api/tokens/evm/erc721/:address/items",
            get(get_nft_metadata_items_handler),
        )
        .with_state(state.clone()) // Use the shared state
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        ;

    let addr = SocketAddr::from(([127, 0, 0, 1], state.config.port));
    info!("🚀 HTTP Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

// --- MCP Server Logic ---
async fn run_mcp_server(state: AppState) {
    info!("🚀 Starting MCP server on stdin/stdout...");

    let mut stdin = io::BufReader::new(io::stdin());
    let mut stdout = io::stdout();

    loop {
        let mut line = String::new();

        match stdin.read_line(&mut line).await {
            Ok(0) => {
                info!("EOF received, shutting down MCP server");
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                debug!("Received: {}", line);

                let response = match serde_json::from_str::<Request>(line) {
                    Ok(request) => {
                        // FIX: Pass shared state to the handler.
                        handle_mcp_request(request, state.clone()).await
                    }
                    Err(parse_error) => {
                        error!("JSON parse error: {}", parse_error);
                        Some(Response::error(
                            serde_json::Value::Null,
                            error_codes::PARSE_ERROR,
                            format!("Parse error: {}", parse_error),
                        ))
                    }
                };

                if let Some(response) = response {
                    if let Ok(response_json) = serde_json::to_string(&response) {
                        debug!("Sending: {}", response_json);
                        if let Err(e) = stdout
                            .write_all(format!("{}\n", response_json).as_bytes())
                            .await
                        {
                            error!("Failed to write response: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }

    info!("MCP server shutting down");
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sei_mcp_server_rs=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // FIX: Load config and handle potential errors gracefully.
    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("❌ Failed to load configuration: {:?}", e);
            return;
        }
    };

    // FIX: Initialize all shared state here, once.
    let sei_client = SeiClient::new(&config.chain_rpc_urls, &config.websocket_url);
    let nonce_manager = NonceManager::new();

    // Initialize wallet storage path but don't require master password on startup
    let wallet_storage_path = match get_wallet_storage_path() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get wallet storage path: {}", e);
            return;
        }
    };

    // Create empty wallet storage - will be initialized when user first registers a wallet
    let storage = WalletStorage::default();

    let app_state = AppState {
        config,
        sei_client,
        nonce_manager,
        wallet_storage: Arc::new(Mutex::new(storage)),
        wallet_storage_path: Arc::new(wallet_storage_path),
    };

    // Determine run mode
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--mcp".to_string()) || env::var("MCP_MODE").is_ok() {
        run_mcp_server(app_state).await;
    } else {
        run_http_server(app_state).await;
    }
}
