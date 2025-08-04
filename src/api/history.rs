use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{blockchain::client::SeiClient, blockchain::models::Transaction, config::AppConfig};

// --- Request and Response Models ---

/// Defines the structure for the path parameters for transaction history.
#[derive(Debug, Deserialize)]
pub struct HistoryPath {
    pub chain_id: String,
    pub address: String,
}

/// Defines the structure for the JSON output of the transaction history API.
#[derive(Debug, Serialize)]
pub struct HistoryOutput {
    pub address: String,
    pub transactions: Vec<Transaction>,
}

// --- Handler ---

/// Handler for the GET /history/{chain_id}/{address} endpoint.
/// This function retrieves the transaction history for an address on a specified chain.
pub async fn get_transaction_history_handler(
    Path(path): Path<HistoryPath>,
    State(config): State<AppConfig>,
) -> Result<Json<HistoryOutput>, (axum::http::StatusCode, String)> {
    info!(
        "Received request for transaction history for chain '{}' and address '{}'",
        path.chain_id, path.address
    );

    let client = SeiClient::new(&config.chain_rpc_urls);

    match client
        .get_transaction_history(&path.chain_id, &path.address)
        .await
    {
        Ok(history_response) => {
            let output = HistoryOutput {
                address: path.address,
                transactions: history_response.transactions,
            };
            Ok(Json(output))
        }
        Err(e) => {
            error!(
                "Failed to get transaction history for {}: {:?}",
                path.address, e
            );
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch transaction history: {}", e),
            ))
        }
    }
}
