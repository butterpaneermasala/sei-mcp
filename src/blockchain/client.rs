// src/blockchain/client.rs

use crate::blockchain::models::{
    BalanceResponse, EstimateFeesRequest, EstimateFeesResponse, ImportWalletError,
    SeiTransferRequest, TransactionHistoryResponse, TransactionResponse, WalletGenerationError,
    WalletResponse,
};
use crate::blockchain::services::balance as balance_service;
use crate::blockchain::services::fees as fees_service;
use crate::blockchain::services::history as history_service;
use crate::blockchain::services::transactions;
use crate::blockchain::services::wallet as wallet_service;
use anyhow::{anyhow, Result};
use reqwest::Client as ReqwestClient;
use std::collections::HashMap;
use tendermint_rpc::client::websocket;
use tendermint_rpc::{
    client::Client as TendermintClient, client::HttpClient, client::WebSocketClient, Order,
};

// --- SeiClient Implementation ---

#[derive(Clone)]
pub struct SeiClient {
    client: ReqwestClient,
    rpc_urls: HashMap<String, String>,
    pub websocket_url: String,
}

impl SeiClient {
    /// Constructor for creating a new `SeiClient`.
    /// It takes a HashMap of chain_id -> rpc_url.
    pub fn new(rpc_urls: &HashMap<String, String>, websocket_url: &str) -> Self {
        Self {
            client: ReqwestClient::new(),
            rpc_urls: rpc_urls.clone(),
            websocket_url: websocket_url.to_string(),
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
        // Support both 'sei' and 'sei-testnet' chains
        if chain_id != "sei" && chain_id != "sei-testnet" {
            return Err(anyhow!(
                "Transaction history via Seistream API is only supported for 'sei' and 'sei-testnet' chains."
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

    /// Transfer SEI tokens to a specified address.
    pub async fn transfer_sei(
        &self,
        chain_id: &str,
        request: &SeiTransferRequest,
    ) -> Result<TransactionResponse> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        transactions::transfer_sei(&self.client, rpc_url, request, &request.private_key).await
    }
    /// Searches for transactions matching a given query.
    pub async fn search_txs(
        &self,
        query: String,
        page: u32,
        per_page: u8,
        order: Order,
    ) -> Result<tendermint_rpc::endpoint::tx_search::Response, anyhow::Error> {
        // Try to get RPC URL for either 'sei' or 'sei-testnet' chain
        let rpc_url = self
            .rpc_urls
            .get("sei")
            .or_else(|| self.rpc_urls.get("sei-testnet"))
            .ok_or_else(|| anyhow!("No RPC URL found for 'sei' or 'sei-testnet' chain"))?;

        let http_client = HttpClient::new(rpc_url.as_str())
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        let response = http_client
            .tx_search(
                query.parse().map_err(|e| anyhow!("Invalid query: {}", e))?,
                false, // prove
                page,
                per_page,
                order,
            )
            .await
            .map_err(|e| anyhow!("RPC error on tx_search: {}", e))?;

        Ok(response)
    }

    /// Returns a WebSocket client for event subscriptions.
    pub async fn get_subscription_client(&self) -> Result<WebSocketClient, anyhow::Error> {
        let (client, driver) = WebSocketClient::new(self.websocket_url.as_str())
            .await
            .map_err(|e| anyhow!("Failed to connect to WebSocket: {}", e))?;

        // The driver task handles the connection and must be spawned.
        tokio::spawn(driver.run());

        Ok(client)
    }
}
