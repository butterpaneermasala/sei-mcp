use anyhow::Result;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    blockchain::client::SeiClient,
    config::AppConfig,
};

// --- Request and Response Models ---

/// Defines the structure for the JSON output when creating a wallet.
#[derive(Debug, Serialize)]
pub struct WalletOutput {
    pub address: String,
    pub private_key: String,
    pub mnemonic: String,
}

/// Defines the structure for the JSON input when importing a wallet.
#[derive(Debug, Deserialize)]
pub struct ImportWalletInput {
    pub mnemonic_or_private_key: String,
}

/// Defines the structure for the JSON output when importing a wallet.
#[derive(Debug, Serialize)]
pub struct ImportWalletOutput {
    pub address: String,
    pub private_key: String,
}

// --- Handlers ---

/// Handler for the POST /wallet/create endpoint.
/// This function generates a new HD wallet, including a mnemonic, and returns the details.
pub async fn create_wallet_handler(
    State(config): State<AppConfig>,
) -> Result<Json<WalletOutput>, (axum::http::StatusCode, String)> {
    info!("Received request to create a new wallet.");

    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    match client.create_wallet().await {
        Ok(wallet) => {
            let output = WalletOutput {
                address: wallet.address,
                private_key: wallet.private_key,
                mnemonic: wallet.mnemonic.unwrap_or_else(|| "N/A".to_string()),
            };
            Ok(Json(output))
        }
        Err(e) => {
            error!("Failed to create wallet: {:?}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create wallet".to_string(),
            ))
        }
    }
}

/// Handler for the POST /wallet/import endpoint.
/// This function imports a wallet from a mnemonic or private key and returns the wallet details.
pub async fn import_wallet_handler(
    State(config): State<AppConfig>,
    Json(payload): Json<ImportWalletInput>,
) -> Result<Json<ImportWalletOutput>, (axum::http::StatusCode, String)> {
    info!("Received request to import a wallet.");

    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    match client.import_wallet(&payload.mnemonic_or_private_key).await {
        Ok(wallet) => {
            let output = ImportWalletOutput {
                address: wallet.address,
                private_key: wallet.private_key,
            };
            Ok(Json(output))
        }
        Err(e) => {
            error!("Failed to import wallet: {:?}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to import wallet: {}", e),
            ))
        }
    }
}
