use anyhow::Result;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    AppState,
    blockchain::models::ChainType,
};

// --- Request and Response Models ---

/// Defines the structure for the JSON output when creating a wallet.
#[derive(Debug, Serialize)]
pub struct WalletOutput {
    pub address: String,
    // The mnemonic is only returned on creation and should be handled securely by the client.
    pub mnemonic: String,
}

/// Defines the structure for the JSON input when creating a wallet, allowing network specification.
#[derive(Debug, Deserialize, Default)]
pub struct CreateWalletInput {
    pub chain_type: Option<String>,
}

/// Defines the structure for the JSON input when importing a wallet.
#[derive(Debug, Deserialize)]
pub struct ImportWalletInput {
    pub mnemonic_or_private_key: String,
    pub chain_type: Option<String>,
}

/// Defines the structure for the JSON output when importing a wallet.
#[derive(Debug, Serialize)]
pub struct ImportWalletOutput {
    pub address: String,
}

// --- Handlers ---

/// Handler for the POST /wallet/create endpoint.
pub async fn create_wallet_handler(
    State(_state): State<AppState>,
    Json(input): Json<CreateWalletInput>,
) -> Result<Json<WalletOutput>, (axum::http::StatusCode, String)> {
    info!("Handling wallet creation request for chain_type: {:?}", input.chain_type);

    let chain_type_str = input.chain_type.unwrap_or_else(|| "evm".to_string());
    let chain_type = match chain_type_str.as_str() {
        "native" => ChainType::Native,
        "evm" => ChainType::Evm,
        _ => return Err((axum::http::StatusCode::BAD_REQUEST, "Invalid chain_type. Use 'native' or 'evm'.".to_string())),
    };

    match crate::blockchain::services::wallet::create_wallet_for_network(chain_type) {
        Ok(wallet) => Ok(Json(WalletOutput {
            address: wallet.address,
            mnemonic: wallet.mnemonic.unwrap_or_default(),
        })),
        Err(e) => {
            error!("Failed to create wallet: {}", e);
            Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Handler for the POST /wallet/import endpoint.
pub async fn import_wallet_handler(
    State(_state): State<AppState>,
    Json(input): Json<ImportWalletInput>,
) -> Result<Json<ImportWalletOutput>, (axum::http::StatusCode, String)> {
    info!("Handling wallet import request for chain_type: {:?}", input.chain_type);

    let chain_type_str = input.chain_type.unwrap_or_else(|| "evm".to_string());
    let chain_type = match chain_type_str.as_str() {
        "native" => ChainType::Native,
        "evm" => ChainType::Evm,
        _ => return Err((axum::http::StatusCode::BAD_REQUEST, "Invalid chain_type. Use 'native' or 'evm'.".to_string())),
    };

    match crate::blockchain::services::wallet::import_wallet_for_network(chain_type, &input.mnemonic_or_private_key) {
        Ok(wallet) => Ok(Json(ImportWalletOutput {
            address: wallet.address,
        })),
        Err(e) => {
            error!("Failed to import wallet: {}", e);
            Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}