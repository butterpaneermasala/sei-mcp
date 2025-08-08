use crate::blockchain::models::BalanceResponse;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, error, info};

pub async fn get_balance(client: &Client, rpc_url: &str, address: &str) -> Result<BalanceResponse> {
    info!(
        "Attempting to fetch balance for address: {} on rpc_url: {}",
        address, rpc_url
    );

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [address, "latest"],
        "id": 1
    });

    debug!("Sending RPC request to {}: {:?}", rpc_url, payload);

    let res: Value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    debug!("Received RPC response: {:?}", res);

    let result = res["result"]
        .as_str()
        .ok_or_else(|| anyhow!("RPC response missing 'result' field: {:?}", res))?;

    let amount_decimal = u128::from_str_radix(result.trim_start_matches("0x"), 16)
        .map(|val| val.to_string())
        .unwrap_or_else(|_| {
            error!(
                "Failed to parse hex balance '{}' to u128. Defaulting to '0'.",
                result
            );
            "0".to_string()
        });

    Ok(BalanceResponse {
        amount: amount_decimal,
        denom: "usei".to_string(),
    })
}
