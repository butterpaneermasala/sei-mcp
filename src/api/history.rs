use axum::{
    Json,
    extract::{Path, Query, State},
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

/// Defines the structure for the query parameters for transaction history.
/// We use Option<u64> to make the parameter optional.
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u64>,
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
///
/// It now accepts an optional `range` query parameter to specify the number of blocks to scan.
/// Example: GET /history/sei/0x...address...?range=5000
pub async fn get_transaction_history_handler(
    Path(path): Path<HistoryPath>,
    Query(query): Query<HistoryQuery>, // <-- New Query extractor
    State(config): State<AppConfig>,
) -> Result<Json<HistoryOutput>, (axum::http::StatusCode, String)> {
    info!(
        "Received request for transaction history for chain '{}' and address '{}'",
        path.chain_id, path.address
    );

    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    // Use the provided range or a default value (e.g., 2000 blocks).
    let limit = query.limit.unwrap_or(20); // Default to 20 transactions
    match client
        .get_transaction_history(&path.chain_id, &path.address, limit)
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
