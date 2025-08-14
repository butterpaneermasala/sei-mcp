// src/api/faucet.rs

use crate::blockchain::services::faucet::send_faucet_tokens;
use crate::config::Config;
use axum::debug_handler;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct FaucetRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct FaucetResponse {
    pub success: bool,
    #[serde(rename = "txHash")]
    pub tx_hash: String,
}

/// Axum handler for the faucet request endpoint.
/// It now only accepts EVM addresses.
#[debug_handler]
pub async fn request_faucet(
    State(config): State<Config>,
    Json(req): Json<FaucetRequest>,
) -> Result<Json<FaucetResponse>, (StatusCode, String)> {
    // Validate that the provided address is an EVM address (starts with "0x").
    if !req.address.starts_with("0x") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid address format. Only EVM (0x...) addresses are supported for the faucet.".to_string(),
        ));
    }

    // Call the underlying service to send the tokens.
    match send_faucet_tokens(&config, &req.address).await {
        Ok(tx_hash) => Ok(Json(FaucetResponse {
            success: true,
            tx_hash,
        })),
        Err(e) => {
            tracing::error!("Faucet transaction failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Faucet transaction failed: {}", e),
            ))
        }
    }
}
