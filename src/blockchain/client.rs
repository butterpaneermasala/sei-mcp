// src/blockchain/client.rs

use reqwest::Client; // HTTP client for making requests.
use serde_json::{json, Value}; // For constructing and parsing JSON payloads.
use crate::blockchain::models::BalanceResponse; // Import our balance data model.
use anyhow::{anyhow, Result}; // For simplified error handling.
use tracing::{debug, info, error}; // For logging.

pub struct SeiClient {
    client: Client, // An instance of the Reqwest HTTP client.
    rpc_url: String, // The URL of the Sei RPC endpoint.
}

impl SeiClient {
    // Constructor for creating a new `SeiClient`.
    pub fn new(rpc_url: &str) -> Self {
        Self {
            client: Client::new(), // Initialize a new Reqwest client.
            rpc_url: rpc_url.to_string(), // Store the RPC URL.
        }
    }

    // Asynchronous function to get the native balance of an address.
    // This simulates an EVM JSON-RPC call.
    pub async fn get_balance(&self, address: &str) -> Result<BalanceResponse> {
        info!("Attempting to fetch balance for address: {}", address);

        // Construct the JSON-RPC payload for `eth_getBalance`.
        // This is a common method in EVM-compatible chains to get the native token balance.
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [address, "latest"], // "latest" refers to the current block.
            "id": 1
        });

        debug!("Sending RPC request to {}: {:?}", self.rpc_url, payload);

        // Send the POST request to the RPC endpoint with the JSON payload.
        let res: Value = self.client
            .post(&self.rpc_url)
            .json(&payload) // Serialize the payload to JSON.
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send RPC request: {}", e);
                anyhow!("Failed to send RPC request: {}", e)
            })?
            .json() // Deserialize the response body to a `serde_json::Value`.
            .await
            .map_err(|e| {
                error!("Failed to parse RPC response JSON: {}", e);
                anyhow!("Failed to parse RPC response JSON: {}", e)
            })?;

        debug!("Received RPC response: {:?}", res);

        // Extract the "result" field from the JSON response.
        // This is expected to be a hexadecimal string representing the balance in wei.
        let result = res["result"]
            .as_str()
            .ok_or_else(|| {
                error!("RPC response missing 'result' field or not a string: {:?}", res);
                anyhow!("RPC response missing 'result' or not a string")
            })?;

        // Convert the hexadecimal balance string to a decimal string.
        // `trim_start_matches("0x")` removes the "0x" prefix.
        // `u128::from_str_radix` parses a string in a given base (here, 16 for hex).
        let amount_decimal = u128::from_str_radix(result.trim_start_matches("0x"), 16)
            .map(|val| val.to_string()) // Convert the u128 to a String.
            .unwrap_or_else(|_| {
                error!("Failed to parse hex balance '{}' to u128. Defaulting to '0'.", result);
                "0".to_string() // Default to "0" if parsing fails.
            });

        // Return the `BalanceResponse`.
        // In a real application, you'd likely convert `wei` to a more human-readable unit (e.g., SEI)
        // by dividing by 10^18 or similar, depending on the token's decimals.
        Ok(BalanceResponse {
            amount: amount_decimal,
            denom: "wei".to_string(), // Indicate the unit of the balance.
        })
    }
}
