// src/blockchain/client.rs

use crate::blockchain::models::{
    BalanceResponse, EstimateFeesRequest, EstimateFeesResponse, ImportWalletError,
    TransactionHistoryResponse, WalletGenerationError, WalletResponse,
};
use crate::blockchain::services::balance as balance_service;
use crate::blockchain::services::fees as fees_service;
use crate::blockchain::services::history as history_service;
use crate::blockchain::services::wallet as wallet_service;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::collections::HashMap;

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
        let rpc_url = self.get_rpc_url(chain_id)?;
        balance_service::get_balance(&self.client, rpc_url, address).await
    }

    /// Generates a new HD wallet with a mnemonic.
    pub async fn create_wallet(&self) -> Result<WalletResponse, WalletGenerationError> {
        wallet_service::create_wallet()
    }

    /// Imports an existing wallet from a private key or mnemonic.
    pub async fn import_wallet(&self, input: &str) -> Result<WalletResponse, ImportWalletError> {
        wallet_service::import_wallet(input)
    }

    /// Retrieves transaction history by scanning a specified range of recent blocks.
    ///
    /// NOTE: This is a resource-intensive operation. It is recommended to use a small
    /// block_scan_range for responsiveness or a dedicated indexer service for production.
    pub async fn get_transaction_history(
        &self,
        chain_id: &str,
        address: &str,
        limit: u64,
    ) -> Result<TransactionHistoryResponse> {
        // This implementation is now specific to the 'sei' chain
        if chain_id != "sei" {
            return Err(anyhow!(
                "Transaction history via Seistream API is only supported for the 'sei' chain."
            ));
        }
        // The rpc_url is no longer needed for the new history service
        history_service::get_transaction_history(&self.client, address, limit).await
    }

    /// Estimates the gas fees for a given transaction.
    pub async fn estimate_fees(
        &self,
        chain_id: &str,
        request: &EstimateFeesRequest,
    ) -> Result<EstimateFeesResponse> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        fees_service::estimate_fees(&self.client, rpc_url, request).await
    }
}
