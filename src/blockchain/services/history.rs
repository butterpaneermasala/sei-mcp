// src/blockchain/client.rs
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use ethers_core::types::U256;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::str::FromStr;
use tracing::{info, warn, error};

use crate::blockchain::models::{Transaction, TransactionHistoryResponse, TransactionType};

// --- Helper Structs for Deserialization ---
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Block {
    timestamp: String,
    transactions: Vec<TransactionObject>,
}

#[derive(Deserialize, Debug, Clone)]
struct TransactionObject {
    hash: String,
    from: String,
    to: Option<String>,
    value: String,
    // Add other fields you might need
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcLog {
    address: String,
    topics: Vec<String>,
    data: String,
    block_number: String,
    transaction_hash: String,
    timestamp: Option<String>, // We'll add this later
}

// Keccak-256 hash of the ERC20 Transfer event signature: `Transfer(address,address,uint256)`
// To get this: `keccak256("Transfer(address,address,uint256)")`
const TRANSFER_EVENT_SIGNATURE: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

pub async fn get_transaction_history(
    client: &Client,
    rpc_url: &str,
    address: &str,
    block_scan_range: u64,
) -> Result<TransactionHistoryResponse> {
    info!(
        "Scanning recent blocks for transaction history for address: {} on rpc_url: {}",
        address, rpc_url
    );
    const CONCURRENT_REQUESTS: usize = 10;
    
    let target_address_lower = address.to_lowercase();
    let latest_block_number = get_latest_block_number(client, rpc_url).await?;
    let start_block = latest_block_number.saturating_sub(block_scan_range);

    // Get Native Transfers
    let native_transfers = get_native_transfers(
        client,
        rpc_url,
        &target_address_lower,
        start_block,
        latest_block_number,
        CONCURRENT_REQUESTS,
    ).await?;
    
    // Get ERC20 Transfers
    let erc20_transfers = get_erc20_transfers(
        client,
        rpc_url,
        &target_address_lower,
        start_block,
        latest_block_number,
    ).await?;

    let mut all_transactions = native_transfers;
    all_transactions.extend(erc20_transfers);

    // Sort all transactions by timestamp descending
    all_transactions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(TransactionHistoryResponse {
        transactions: all_transactions,
    })
}

// Fetches native token transfers
async fn get_native_transfers(
    client: &Client,
    rpc_url: &str,
    target_address_lower: &str,
    start_block: u64,
    latest_block: u64,
    concurrent_requests: usize,
) -> Result<Vec<Transaction>> {
    let block_numbers: Vec<u64> = (start_block..=latest_block).collect();

    let bodies = stream::iter(block_numbers)
        .map(|block_num| {
            let client = client.clone();
            let rpc_url = rpc_url.to_string();
            async move {
                let block_hex = format!("0x{:x}", block_num);
                let payload = json!({
                    "jsonrpc": "2.0",
                    "method": "eth_getBlockByNumber",
                    "params": [block_hex, true],
                    "id": 1
                });
                match client.post(&rpc_url).json(&payload).send().await {
                    Ok(resp) => match resp.json::<Value>().await {
                        Ok(val) => Ok((val, block_num)),
                        Err(e) => Err(anyhow!("Failed to parse block JSON: {}", e)),
                    },
                    Err(e) => Err(anyhow!("RPC request failed for block {}: {}", block_num, e)),
                }
            }
        })
        .buffer_unordered(concurrent_requests);

    let results: Vec<Transaction> = bodies
        .filter_map(|res| async {
            match res {
                Ok((val, block_num)) => {
                    if let Some(result_obj) = val.get("result") {
                        if result_obj.is_null() {
                            return None;
                        }
                        match serde_json::from_value::<Block>(result_obj.clone()) {
                            Ok(block) => Some((block, target_address_lower.to_string())),
                            Err(e) => {
                                warn!(
                                    "Failed to deserialize block {}: {}. Value: {}",
                                    block_num, e, result_obj
                                );
                                None
                            }
                        }
                    } else {
                        warn!("RPC response missing 'result' field for block {}: {:?}", block_num, val);
                        None
                    }
                }
                Err(e) => {
                    error!("Error fetching block: {:?}", e);
                    None
                }
            }
        })
        .flat_map(|(block, target_addr)| {
            let timestamp_u64 =
                u64::from_str_radix(block.timestamp.trim_start_matches("0x"), 16).unwrap_or(0);
            let datetime =
                DateTime::<Utc>::from_timestamp(timestamp_u64 as i64, 0).unwrap_or_default();

            let transactions = block
                .transactions
                .into_iter()
                .filter(move |tx| {
                    let is_target_from = tx.from.to_lowercase() == target_addr;
                    let is_target_to = tx.to.as_deref().unwrap_or("").to_lowercase() == target_addr;
                    // Only include native transfers with non-zero value
                    (is_target_from || is_target_to) && tx.value != "0x0"
                })
                .map(move |tx| {
                    let amount = U256::from_str_radix(tx.value.trim_start_matches("0x"), 16)
                        .unwrap_or_default()
                        .to_string();

                    Transaction {
                        tx_hash: tx.hash,
                        from_address: tx.from,
                        to_address: tx.to.unwrap_or_else(|| "N/A".to_string()),
                        amount,
                        denom: "usei".to_string(),
                        timestamp: datetime.to_rfc3339(),
                        transaction_type: TransactionType::Native,
                        contract_address: None,
                    }
                });
            stream::iter(transactions)
        })
        .collect()
        .await;
    
    Ok(results)
}

// Fetches ERC20 token transfers using eth_getLogs
async fn get_erc20_transfers(
    client: &Client,
    rpc_url: &str,
    target_address_lower: &str,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Transaction>> {
    // The RPC call `eth_getLogs` returns logs for all transfers,
    // so we must filter for our target address in the topics.
    let from_topic = format!("0x000000000000000000000000{}", target_address_lower.trim_start_matches("0x"));
    let to_topic = format!("0x000000000000000000000000{}", target_address_lower.trim_start_matches("0x"));

    // We can't filter on both `from` AND `to` topics simultaneously in a single call,
    // so we'll do two separate queries and merge the results.
    let payload_from = json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
            "topics": [
                TRANSFER_EVENT_SIGNATURE,
                from_topic
            ]
        }],
        "id": 1
    });

    let payload_to = json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
            "topics": [
                TRANSFER_EVENT_SIGNATURE,
                Value::Null, // The second topic is `from`, which we don't care about here
                to_topic
            ]
        }],
        "id": 1
    });

    // Execute both requests concurrently
    let (res_from, res_to) = futures::join!(
        client.post(rpc_url).json(&payload_from).send(),
        client.post(rpc_url).json(&payload_to).send()
    );

    let mut logs = Vec::new();
    let res_from = res_from?.json::<Value>().await?;
    let res_to = res_to?.json::<Value>().await?;

    if let Some(result) = res_from.get("result") {
        let from_logs: Vec<RpcLog> = serde_json::from_value(result.clone())?;
        logs.extend(from_logs);
    }
    if let Some(result) = res_to.get("result") {
        let to_logs: Vec<RpcLog> = serde_json::from_value(result.clone())?;
        // Filter out duplicates (transactions where from and to are the same address)
        for log in to_logs {
            if !logs.iter().any(|existing| existing.transaction_hash == log.transaction_hash) {
                logs.push(log);
            }
        }
    }

    // Fetch block timestamps for all logs
    let block_numbers: Vec<u64> = logs
        .iter()
        .map(|log| u64::from_str_radix(log.block_number.trim_start_matches("0x"), 16).unwrap_or(0))
        .collect();

    let block_timestamps: Vec<Result<String>> = stream::iter(block_numbers)
        .map(|block_num| {
            let client = client.clone();
            let rpc_url = rpc_url.to_string();
            async move { get_block_timestamp(client, &rpc_url, block_num).await }
        })
        .buffer_unordered(10) // Limit concurrent requests
        .collect()
        .await;

    // Combine logs with their timestamps
    let mut logs_with_timestamps: Vec<RpcLog> = logs;
    for (log, ts) in logs_with_timestamps.iter_mut().zip(block_timestamps.into_iter()) {
        if let Ok(timestamp) = ts {
            log.timestamp = Some(timestamp);
        } else {
            warn!("Failed to get timestamp for block");
        }
    }
    
    // Process logs into our Transaction model
    let results: Vec<Transaction> = logs_with_timestamps
        .into_iter()
        .filter_map(|log| {
            // A `Transfer` event has 3 topics: signature, from, to
            if log.topics.len() < 3 {
                return None;
            }
            let from_address = format!("0x{}", log.topics[1].trim_start_matches("0x").chars().skip(24).collect::<String>());
            let to_address = format!("0x{}", log.topics[2].trim_start_matches("0x").chars().skip(24).collect::<String>());
            
            let amount = U256::from_str_radix(log.data.trim_start_matches("0x"), 16)
                .unwrap_or_default()
                .to_string();

            // Note: This is an incomplete implementation for denom. A full solution would require
            // another RPC call to the contract address to get the token symbol.
            // For now, we'll just use a placeholder.
            let denom = "ERC20".to_string();

            let timestamp = log.timestamp.unwrap_or_else(|| Utc::now().to_rfc3339());

            Some(Transaction {
                tx_hash: log.transaction_hash,
                from_address,
                to_address,
                amount,
                denom,
                timestamp,
                transaction_type: TransactionType::ERC20,
                contract_address: Some(log.address),
            })
        })
        .collect();

    Ok(results)
}

// Fetches the timestamp for a single block
async fn get_block_timestamp(
    client: Client,
    rpc_url: &str,
    block_num: u64,
) -> Result<String> {
    let block_hex = format!("0x{:x}", block_num);
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": [block_hex, false],
        "id": 1
    });
    
    let resp: Value = client.post(rpc_url).json(&payload).send().await?.json().await?;
    let timestamp_hex = resp["result"]["timestamp"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get block timestamp: invalid response"))?;
    
    let timestamp_u64 = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)?;
    let datetime = DateTime::<Utc>::from_timestamp(timestamp_u64 as i64, 0)
        .ok_or_else(|| anyhow!("Failed to parse timestamp"))?;
    
    Ok(datetime.to_rfc3339())
}


async fn get_latest_block_number(client: &Client, rpc_url: &str) -> Result<u64> {
    // (This function remains the same as your original implementation)
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });
    let res: Value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;
    let block_hex = res["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get latest block number: invalid response"))?;
    Ok(u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)?)
}