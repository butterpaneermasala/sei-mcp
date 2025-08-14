// src/main.rs

use axum::{routing::get, routing::post, Router};
use sei_mcp_server_rs::{
    api::{
        balance::get_balance_handler,
        event::{get_contract_events, search_events, subscribe_contract_events},
        faucet::request_faucet,
        fees::estimate_fees_handler,
        health::health_handler,
        history::get_transaction_history_handler,
        transfer::transfer_sei_handler,
        wallet::{create_wallet_handler, import_wallet_handler},
    },
    config::Config,
    mcp::{
        handler::handle_mcp_request,
        protocol::{error_codes, Request, Response},
    },
};
use std::env;
use std::net::SocketAddr;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// --- HTTP Server Logic ---
async fn run_http_server(config: Config) {
    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/wallet/create", post(create_wallet_handler))
        .route("/api/wallet/import", post(import_wallet_handler))
        .route("/api/balance/:chain_id/:address", get(get_balance_handler))
        .route("/api/history/:chain_id/:address", get(get_transaction_history_handler))
        .route("/api/transfer/:chain_id", post(transfer_sei_handler))
        .route("/api/fees/estimate/:chain_id", post(estimate_fees_handler))
        .route("/api/events/search", get(search_events))
        .route("/api/events/contract", get(get_contract_events))
        .route("/api/events/subscribe", get(subscribe_contract_events))
        .route("/api/faucet/request", post(request_faucet))
        .with_state(config.clone())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    info!("🚀 HTTP Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- MCP Server Logic ---
async fn run_mcp_server(config: Config) {
    info!("🚀 Starting MCP server on stdin/stdout...");
    
    let mut stdin = io::BufReader::new(io::stdin());
    let mut stdout = io::stdout();

    loop {
        let mut line = String::new();
        
        match stdin.read_line(&mut line).await {
            Ok(0) => {
                debug!("EOF received, shutting down MCP server");
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                
                debug!("Received: {}", line);
                
                // Parse the request with better error handling
                let response = match serde_json::from_str::<Request>(line) {
                    Ok(request) => {
                        // Validate the request has required fields
                        if request.jsonrpc != "2.0" {
                            Some(Response::error(
                                request.id,
                                error_codes::INVALID_REQUEST,
                                "Invalid jsonrpc version, must be '2.0'".to_string(),
                            ))
                        } else if request.method.is_empty() {
                            Some(Response::error(
                                request.id,
                                error_codes::INVALID_REQUEST,
                                "Missing required 'method' field".to_string(),
                            ))
                        } else {
                            // Handle the valid request
                            handle_mcp_request(request, &config).await
                        }
                    }
                    Err(parse_error) => {
                        error!("JSON parse error: {}", parse_error);
                        // For parse errors, we can't get a valid ID, so use null
                        Some(Response::error(
                            serde_json::Value::Null,
                            error_codes::PARSE_ERROR,
                            format!("Parse error: {}", parse_error),
                        ))
                    }
                };

                // Send response if there is one
                if let Some(response) = response {
                    match serde_json::to_string(&response) {
                        Ok(response_json) => {
                            debug!("Sending: {}", response_json);
                            if let Err(e) = stdout.write_all(format!("{}\n", response_json).as_bytes()).await {
                                error!("Failed to write response: {}", e);
                                break;
                            }
                            if let Err(e) = stdout.flush().await {
                                error!("Failed to flush stdout: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize response: {}", e);
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

    // Load configuration
    let config = Config::from_env().expect("Failed to load configuration");

    // Determine run mode
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--mcp".to_string()) || env::var("MCP_MODE").is_ok() {
        run_mcp_server(config).await;
    } else {
        run_http_server(config).await;
    }
}