use crate::{
    blockchain::client::SeiClient, config::AppConfig,
};
use anyhow::Result;
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use tracing::error;

// Defines the structure for the address and chain_id extracted from the URL path.
#[derive(Debug, Deserialize)]
pub struct BalancePath {
    pub chain_id: String,
    pub address: String,
}

// Defines the structure for the JSON output returned by our API.
#[derive(Debug, Serialize)]
pub struct BalanceOutput {
    pub chain_id: String,
    pub address: String,
    pub balance: String,
    pub denom: String,
}

// The handler function for the GET /balance/{chain_id}/{address} endpoint.
pub async fn get_balance_handler(
    Path(path): Path<BalancePath>,
    State(config): State<AppConfig>,
) -> Result<Json<BalanceOutput>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    match client.get_balance(&path.chain_id, &path.address).await {
        Ok(balance_response) => {
            let output = BalanceOutput {
                chain_id: path.chain_id,
                address: path.address,
                balance: balance_response.amount,
                denom: balance_response.denom,
            };
            Ok(Json(output))
        }
        Err(e) => {
            error!("Failed to get balance for {}: {:?}", path.address, e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch balance: {}", e),
            ))
        }
    }
}
