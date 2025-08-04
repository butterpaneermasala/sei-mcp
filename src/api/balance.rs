// src/api/balance.rs

use axum::{
    extract::{Path, State}, // `Path` extracts URL path segments, `State` extracts application state.
    Json, // For returning JSON responses.
};
use serde::{Deserialize, Serialize}; // For deriving serialization/deserialization traits.
use crate::{
    config::AppConfig, // Import our application configuration.
    blockchain::client::SeiClient, // Import the Sei blockchain client.
    blockchain::models::BalanceResponse, // Import the data model for a balance response.
};
use anyhow::Result; // For simplified error handling.
use tracing::error; // For logging errors.

// Defines the structure for the address extracted from the URL path.
#[derive(Debug, Deserialize)]
pub struct BalancePath {
    pub address: String,
}

// Defines the structure for the JSON output returned by our API.
// `Serialize` is needed to convert this struct into a JSON string.
#[derive(Debug, Serialize)]
pub struct BalanceOutput {
    pub address: String,
    // Represent balance as a string to avoid floating-point precision issues
    // with large blockchain integer amounts (e.g., wei).
    pub balance: String,
    pub denom: String, // e.g., "wei", "SEI", "USDC"
}

// The handler function for the GET /balance/:address endpoint.
// `Path(path)` extracts the `address` from the URL.
// `State(config)` extracts the `AppConfig` shared across the application.
// It returns a `Result` which either contains a `Json<BalanceOutput>` on success
// or an `(axum::http::StatusCode, String)` tuple on error for HTTP responses.
pub async fn get_balance_handler(
    Path(path): Path<BalancePath>,
    State(config): State<AppConfig>,
) -> Result<Json<BalanceOutput>, (axum::http::StatusCode, String)> {
    // Create a new SeiClient instance using the RPC URL from our config.
    let client = SeiClient::new(&config.sei_rpc_url);

    // Attempt to fetch the balance for the given address.
    match client.get_balance(&path.address).await {
        Ok(balance_response) => {
            // If successful, construct the desired output format.
            let output = BalanceOutput {
                address: path.address,
                balance: balance_response.amount,
                denom: balance_response.denom,
            };
            // Return the output as a JSON response with a 200 OK status.
            Ok(Json(output))
        },
        Err(e) => {
            // If an error occurs, log it and return an appropriate HTTP error response.
            error!("Failed to get balance for {}: {:?}", path.address, e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch balance: {}", e),
            ))
        }
    }
}
