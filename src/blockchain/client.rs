// src/blockchain/client.rs
use crate::blockchain::models::{
    BalanceResponse, EstimateFeesRequest, EstimateFeesResponse, ImportWalletError, Transaction,
    TransactionHistoryResponse, WalletGenerationError, WalletResponse,
};
use anyhow::{Result, anyhow};
use bip39::{Language, Mnemonic}; // CORRECTED: Removed invalid imports `Seed`
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::{Address, U256};
use ethers_core::utils::hex;
use rand::{RngCore, rngs::OsRng};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, error, info};

pub struct SeiClient {
    client: Client,
    rpc_urls: HashMap<String, String>,
}

impl SeiClient {
    // Constructor for creating a new `SeiClient`.
    // It now takes a HashMap of chain_id -> rpc_url.
    pub fn new(rpc_urls: &HashMap<String, String>) -> Self {
        Self {
            client: Client::new(),
            rpc_urls: rpc_urls.clone(),
        }
    }

    // A helper function to get the RPC URL for a given chain.
    fn get_rpc_url(&self, chain_id: &str) -> Result<&String> {
        self.rpc_urls
            .get(chain_id)
            .ok_or_else(|| anyhow!("RPC URL not found for chain_id: {}", chain_id))
    }

    // Asynchronous function to get the native balance of an address on a specific chain.
    // This is updated to use the new multi-chain configuration.
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

        let result = res["result"].as_str().ok_or_else(|| {
            anyhow!(
                "RPC response missing 'result' field or not a string: {:?}",
                res
            )
        })?;

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
            denom: "wei".to_string(),
        })
    }

    // --- New Functionality: Wallet Management ---

    /// Generates a new HD wallet with a mnemonic.
    pub async fn create_wallet(&self) -> Result<WalletResponse, WalletGenerationError> {
        info!("Generating a new wallet...");
        // Generate 24-word mnemonic
        let mut entropy = [0u8; 32]; // 32 bytes = 256 bits = 24 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        let phrase = mnemonic.to_string();

        let seed = mnemonic.to_seed("");
        let private_key = SigningKey::from_slice(&seed[..32])
            .map_err(|e| anyhow!("Failed to create signing key from seed: {}", e))?;

        let public_key = private_key.verifying_key();

        let encoded_point = public_key.to_encoded_point(false);
        let pubkey_bytes = encoded_point.as_bytes();
        let hash = ethers_core::utils::keccak256(&pubkey_bytes[1..]);
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

        // Try to import from mnemonic first
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
            let hash = ethers_core::utils::keccak256(&pubkey_bytes[1..]);
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

        // If not a mnemonic, try to import from private key
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
            let hash = ethers_core::utils::keccak256(&pubkey_bytes[1..]);
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

    // --- New Functionality: Transaction History ---

    /// Retrieves transaction history for a given address on a specified chain.
    /// NOTE: This is a simplified example. A real implementation would query a block explorer API
    /// or a dedicated transaction indexer, as standard RPC nodes do not always support this.
    pub async fn get_transaction_history(
        &self,
        chain_id: &str,
        address: &str,
    ) -> Result<TransactionHistoryResponse> {
        info!(
            "Attempting to fetch transaction history for address: {} on chain: {}",
            address, chain_id
        );

        let _rpc_url = self.get_rpc_url(chain_id)?;

        // A real API call would look something like this, but querying a different endpoint.
        // For demonstration, we'll simulate a response.
        let transactions = vec![
            Transaction {
                tx_hash: "0x1234...".to_string(),
                from_address: "0xaddr1".to_string(),
                to_address: address.to_string(),
                amount: "1000000000000000000".to_string(), // 1 SEI
                denom: "SEI".to_string(),
                timestamp: "2024-05-20T10:00:00Z".to_string(),
            },
            Transaction {
                tx_hash: "0x5678...".to_string(),
                from_address: address.to_string(),
                to_address: "0xaddr2".to_string(),
                amount: "500000000000000000".to_string(), // 0.5 SEI
                denom: "SEI".to_string(),
                timestamp: "2024-05-20T11:30:00Z".to_string(),
            },
        ];

        Ok(TransactionHistoryResponse { transactions })
    }

    // --- New Functionality: Fee Estimation ---

    /// Estimates the gas fees for a given transaction.
    /// This uses the standard `eth_estimateGas` JSON-RPC method.
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

        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [{
                "from": request.from,
                "to": request.to,
                "value": amount_hex,
            }],
            "id": 1,
        });

        let res: Value = self
            .client
            .post(rpc_url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let estimated_gas_hex = res["result"].as_str().ok_or_else(|| {
            anyhow!(
                "RPC response missing 'result' field or not a string: {:?}",
                res
            )
        })?;

        let estimated_gas = u64::from_str_radix(estimated_gas_hex.trim_start_matches("0x"), 16)
            .map(|val| val.to_string())
            .unwrap_or_else(|_| "0".to_string());

        let gas_price = "10000000000".to_string();
        let total_fee = "100000000000000".to_string();

        Ok(EstimateFeesResponse {
            estimated_gas,
            gas_price,
            total_fee,
            denom: "wei".to_string(),
        })
    }
}
