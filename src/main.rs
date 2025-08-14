// src/main.rs

use axum::{routing::get, routing::post, Router};
use axum::{middleware, extract::{ConnectInfo, State}};
use sei_mcp_server_rs::AppState;
use sei_mcp_server_rs::{
    api::{
        balance::get_balance_handler,
        faucet::request_faucet,
        health::health_handler,
        history::get_transaction_history_handler,
        tx::send_transaction_handler,
    },
    config::Config,
    mcp::{
        handler::handle_mcp_request,
        protocol::{error_codes, Request, Response},
    },
    blockchain::client::SeiClient,
    blockchain::nonce_manager::NonceManager,
    mcp::wallet_storage::{WalletStorage, get_wallet_storage_path},
};
use sei_mcp_server_rs::api::wallet::{create_wallet_handler, import_wallet_handler};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tower::limit::ConcurrencyLimitLayer;
// removed HandleErrorLayer-based mapping; ConcurrencyLimit is sufficient for now
use axum::response::IntoResponse;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
// Removed rpassword import - no longer needed for startup

// --- HTTP Server Logic ---
async fn run_http_server(state: AppState) {
    // Simple in-memory per-IP rate limiter state
    #[derive(Clone)]
    struct RateLimiter {
        inner: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
        window: Duration,
        max: usize,
    }

    impl RateLimiter {
        fn new(window: Duration, max: usize) -> Self {
            Self { inner: Arc::new(Mutex::new(HashMap::new())), window, max }
        }
    }

    async fn rate_limit_middleware(
        State(limiter): State<RateLimiter>,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        req: axum::http::Request<axum::body::Body>,
        next: axum::middleware::Next,
    ) -> impl axum::response::IntoResponse {
        let path = req.uri().path().to_string();
        let ip = addr.ip().to_string();
        let key = format!("{}::{}", path, ip);
        let now = Instant::now();

        {
            let mut map = limiter.inner.lock().await;
            let entry = map.entry(key).or_default();
            // prune old
            let cutoff = now - limiter.window;
            entry.retain(|t| *t >= cutoff);
            if entry.len() >= limiter.max {
                return (axum::http::StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
            }
            entry.push(now);
        }

        next.run(req).await
    }

    // Build separate limiters from config
    let tx_limiter = RateLimiter::new(
        Duration::from_secs(state.config.tx_rate_window_secs),
        state.config.tx_rate_max,
    );
    let faucet_limiter = RateLimiter::new(
        Duration::from_secs(state.config.faucet_rate_window_secs),
        state.config.faucet_rate_max,
    );

    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/wallet/create", post(create_wallet_handler))
        .route("/api/wallet/import", post(import_wallet_handler))
        .route("/api/balance/:chain_id/:address", get(get_balance_handler))
        .route("/api/history/:chain_id/:address", get(get_transaction_history_handler))
        // Removed estimate_fees and other handlers for brevity, they would follow the same pattern.
        .route(
            "/api/faucet/request",
            post(request_faucet).route_layer(middleware::from_fn_with_state(faucet_limiter.clone(), rate_limit_middleware)),
        )
        .route(
            "/api/tx/send",
            post(send_transaction_handler).route_layer(middleware::from_fn_with_state(tx_limiter.clone(), rate_limit_middleware)),
        )
        .with_state(state.clone()) // Use the shared state
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        // Simple protection: limit concurrent requests
        .layer(ConcurrencyLimitLayer::new(64));

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
                        if let Err(e) = stdout.write_all(format!("{}\n", response_json).as_bytes()).await {
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
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "sei_mcp_server_rs=debug,tower_http=debug".into()))
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
        faucet_cooldowns: Arc::new(Mutex::new(HashMap::new())),
    };

    // Determine run mode
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--mcp".to_string()) || env::var("MCP_MODE").is_ok() {
        run_mcp_server(app_state).await;
    } else {
        run_http_server(app_state).await;
    }
}