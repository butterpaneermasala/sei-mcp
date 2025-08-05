// src/blockchain/client.rs
use crate::blockchain::models::{
    BalanceResponse, EstimateFeesRequest, EstimateFeesResponse, ImportWalletError, Transaction,
    TransactionHistoryResponse, WalletGenerationError, WalletResponse,
};
use anyhow::{Result, anyhow};
use bip39::{Language, Mnemonic};
use chrono::{DateTime, Utc};
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::{Address, U256};
use ethers_core::utils::{hex, keccak256};
use futures::stream::{self, StreamExt};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, error, info, warn};

// --- Helper structs for deserializing RPC responses ---

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
    // 'to' can be null for contract creations
    to: Option<String>,
    value: String,
}

// --- SeiClient Implementation ---

pub struct SeiClient {
    client: Client,
    rpc_urls: HashMap<String, String>,
}

impl SeiClient {
    /// Constructor for creating a new `SeiClient`.
    /// It takes a HashMap of chain_id -> rpc_url.
    pub fn new(rpc_urls: &HashMap<String, String>) -> Self {
        Self {
            client: Client::new(),
            rpc_urls: rpc_urls.clone(),
        }
    }

    /// A helper function to get the RPC URL for a given chain.
    fn get_rpc_url(&self, chain_id: &str) -> Result<&String> {
        self.rpc_urls
            .get(chain_id)
            .ok_or_else(|| anyhow!("RPC URL not found for chain_id: {}", chain_id))
    }

    /// Asynchronous function to get the native balance of an address on a specific chain.
    pub async fn get_balance(&self, chain_id: &str, address: &str) -> Result<BalanceResponse> {
        info!(
            "Attempting to fetch balance for address: {} on chain: {}",
            address, chain_id
        );

        let rpc_url = self.get_rpc_url(chain_id)?;

        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [address, "latest"],
            "id": 1
        });

        debug!("Sending RPC request to {}: {:?}", rpc_url, payload);

        let res: Value = self
            .client
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

    /// Generates a new HD wallet with a mnemonic.
    pub async fn create_wallet(&self) -> Result<WalletResponse, WalletGenerationError> {
        info!("Generating a new wallet...");
        let mut entropy = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        let phrase = mnemonic.to_string();

        let seed = mnemonic.to_seed("");
        let private_key = SigningKey::from_slice(&seed[..32])
            .map_err(|e| anyhow!("Failed to create signing key from seed: {}", e))?;

        let public_key = private_key.verifying_key();
        let encoded_point = public_key.to_encoded_point(false);
        let pubkey_bytes = encoded_point.as_bytes();
        let hash = keccak256(&pubkey_bytes[1..]);
        let address = Address::from_slice(&hash[12..]);

        let private_key_hex = hex::encode(private_key.to_bytes());
        let address_hex = format!("0x{}", hex::encode(address));

        info!("Wallet generated successfully.");
        Ok(WalletResponse {
            address: address_hex,
            private_key: private_key_hex,
            mnemonic: Some(phrase),
        })
    }

    /// Imports an existing wallet from a private key or mnemonic.
    pub async fn import_wallet(&self, input: &str) -> Result<WalletResponse, ImportWalletError> {
        info!("Attempting to import a wallet...");

        if let Ok(mnemonic) = Mnemonic::from_str(input) {
            let seed = mnemonic.to_seed("");
            let private_key = SigningKey::from_slice(&seed[..32]).map_err(|e| {
                ImportWalletError::InvalidMnemonic(format!(
                    "Failed to create signing key from mnemonic: {}",
                    e
                ))
            })?;

            let public_key = private_key.verifying_key();
            let encoded_point = public_key.to_encoded_point(false);
            let pubkey_bytes = encoded_point.as_bytes();
            let hash = keccak256(&pubkey_bytes[1..]);
            let address = Address::from_slice(&hash[12..]);

            let private_key_hex = hex::encode(private_key.to_bytes());
            let address_hex = format!("0x{}", hex::encode(address));

            info!("Wallet imported successfully from mnemonic.");
            return Ok(WalletResponse {
                address: address_hex,
                private_key: private_key_hex,
                mnemonic: Some(input.to_string()),
            });
        }

        if let Ok(private_key_bytes) = hex::decode(input.trim_start_matches("0x")) {
            let private_key = SigningKey::from_slice(&private_key_bytes).map_err(|e| {
                ImportWalletError::InvalidPrivateKey(format!(
                    "Failed to create signing key from private key: {}",
                    e
                ))
            })?;

            let public_key = private_key.verifying_key();
            let encoded_point = public_key.to_encoded_point(false);
            let pubkey_bytes = encoded_point.as_bytes();
            let hash = keccak256(&pubkey_bytes[1..]);
            let address = Address::from_slice(&hash[12..]);

            let private_key_hex = hex::encode(private_key.to_bytes());
            let address_hex = format!("0x{}", hex::encode(address));

            info!("Wallet imported successfully from private key.");
            return Ok(WalletResponse {
                address: address_hex,
                private_key: private_key_hex,
                mnemonic: None,
            });
        }

        error!("Failed to import wallet: input is not a valid mnemonic or private key.");
        Err(ImportWalletError::InvalidPrivateKey(
            "Input is not a valid mnemonic or private key".to_string(),
        ))
    }

    /// Retrieves transaction history by scanning recent blocks directly from the RPC node.
    ///
    /// NOTE: This is a resource-intensive operation. It scans the last `BLOCK_SCAN_RANGE`
    /// blocks to find transactions. For a production system, a dedicated indexer service
    /// would be more efficient.
    pub async fn get_transaction_history(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<TransactionHistoryResponse> {
        info!(
            "Scanning recent blocks for transaction history for address: {} on chain: {}",
            address, chain_id
        );

        const BLOCK_SCAN_RANGE: u64 = 1000; // Scan the last 1000 blocks
        const CONCURRENT_REQUESTS: usize = 10; // Number of concurrent RPC requests

        let rpc_url = self.get_rpc_url(chain_id)?;
        let target_address_lower = address.to_lowercase();

        // 1. Get the latest block number
        let latest_block_number = self.get_latest_block_number(rpc_url).await?;

        // 2. Define the range of blocks to scan
        let start_block = latest_block_number.saturating_sub(BLOCK_SCAN_RANGE);

        // 3. Concurrently fetch blocks and process them
        let block_numbers: Vec<u64> = (start_block..=latest_block_number).collect();

        let bodies = stream::iter(block_numbers)
            .map(|block_num| {
                let client = &self.client;
                let rpc_url = rpc_url.clone();
                async move {
                    let block_hex = format!("0x{:x}", block_num);
                    let payload = json!({
                        "jsonrpc": "2.0",
                        "method": "eth_getBlockByNumber",
                        "params": [block_hex, true], // true for full transaction objects
                        "id": 1
                    });

                    match client.post(&rpc_url).json(&payload).send().await {
                        Ok(resp) => match resp.json::<Value>().await {
                            Ok(val) => Ok(val),
                            Err(e) => Err(anyhow!("Failed to parse block JSON: {}", e)),
                        },
                        Err(e) => Err(anyhow!("RPC request failed for block {}: {}", block_num, e)),
                    }
                }
            })
            .buffer_unordered(CONCURRENT_REQUESTS);

        let results: Vec<Transaction> = bodies
            .filter_map(|res| async {
                match res {
                    Ok(val) => {
                        if let Some(result_obj) = val.get("result") {
                            if result_obj.is_null() {
                                return None;
                            }
                            match serde_json::from_value::<Block>(result_obj.clone()) {
                                Ok(block) => Some((block, target_address_lower.clone())),
                                Err(e) => {
                                    warn!(
                                        "Failed to deserialize block: {}. Value: {}",
                                        e, result_obj
                                    );
                                    None
                                }
                            }
                        } else {
                            warn!("RPC response missing 'result' field: {:?}", val);
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
                        tx.from.to_lowercase() == target_addr
                            || tx.to.as_deref().unwrap_or("").to_lowercase() == target_addr
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
                        }
                    });
                stream::iter(transactions)
            })
            .collect()
            .await;

        Ok(TransactionHistoryResponse {
            transactions: results,
        })
    }

    async fn get_latest_block_number(&self, rpc_url: &str) -> Result<u64> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });
        let res: Value = self
            .client
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

    /// Estimates the gas fees for a given transaction.
    pub async fn estimate_fees(
        &self,
        chain_id: &str,
        request: &EstimateFeesRequest,
    ) -> Result<EstimateFeesResponse> {
        info!(
            "Attempting to estimate fees for a transaction on chain: {}",
            chain_id
        );

        let rpc_url = self.get_rpc_url(chain_id)?;

        let amount = U256::from_dec_str(&request.amount)
            .map_err(|e| anyhow!("Invalid amount format: {}", e))?;
        let amount_hex = format!("0x{:x}", amount);

        let estimate_gas_payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [{
                "from": request.from,
                "to": request.to,
                "value": amount_hex,
            }],
            "id": 1,
        });

        let res_gas: Value = self
            .client
            .post(rpc_url)
            .json(&estimate_gas_payload)
            .send()
            .await?
            .json()
            .await?;

        let estimated_gas_hex = res_gas["result"].as_str().ok_or_else(|| {
            anyhow!(
                "RPC response for estimateGas missing 'result' field: {:?}",
                res_gas
            )
        })?;

        let estimated_gas_u256 =
            U256::from_str_radix(estimated_gas_hex.trim_start_matches("0x"), 16)?;

        let gas_price_payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 2
        });

        let res_price: Value = self
            .client
            .post(rpc_url)
            .json(&gas_price_payload)
            .send()
            .await?
            .json()
            .await?;

        let gas_price_hex = res_price["result"].as_str().ok_or_else(|| {
            anyhow!(
                "RPC response for gasPrice missing 'result' field: {:?}",
                res_price
            )
        })?;

        let gas_price_u256 = U256::from_str_radix(gas_price_hex.trim_start_matches("0x"), 16)?;

        let total_fee_u256 = gas_price_u256
            .checked_mul(estimated_gas_u256)
            .ok_or_else(|| anyhow!("Fee calculation overflow"))?;

        Ok(EstimateFeesResponse {
            estimated_gas: estimated_gas_u256.to_string(),
            gas_price: gas_price_u256.to_string(),
            total_fee: total_fee_u256.to_string(),
            denom: "usei".to_string(),
        })
    }
}
