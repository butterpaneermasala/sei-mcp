// src/blockchain/client.rs

use crate::blockchain::{
    models::*,
    nonce_manager::NonceManager,
    services::{balance, fees, history, transactions, wallet, event},
};
use anyhow::{anyhow, Result};
use ethers_core::types::TransactionRequest;
use std::collections::HashMap;

#[derive(Clone)]
pub struct SeiClient {
    client: reqwest::Client,
    rpc_urls: HashMap<String, String>,
    pub websocket_url: String,
}

impl SeiClient {
    pub fn new(rpc_urls: &HashMap<String, String>, websocket_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_urls: rpc_urls.clone(),
            websocket_url: websocket_url.to_string(),
        }
    }

    pub fn get_rpc_url(&self, chain_id: &str) -> Result<&String> {
        self.rpc_urls
            .get(chain_id)
            .ok_or_else(|| anyhow!("RPC URL not found for chain_id: {}", chain_id))
    }

    pub async fn get_balance(&self, chain_id: &str, address: &str) -> Result<BalanceResponse> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        let is_native = crate::blockchain::models::ChainType::from_chain_id(chain_id)
            == crate::blockchain::models::ChainType::Native;
        balance::get_balance(&self.client, rpc_url, address, is_native).await
    }

    pub async fn create_wallet(&self) -> Result<WalletResponse, WalletGenerationError> {
        wallet::create_wallet()
    }

    pub async fn import_wallet(&self, input: &str) -> Result<WalletResponse, ImportWalletError> {
        wallet::import_wallet(input)
    }

    pub async fn get_transaction_history(
        &self,
        chain_id: &str,
        address: &str,
        limit: u64,
    ) -> Result<TransactionHistoryResponse> {
        if chain_id != "sei" && chain_id != "sei-testnet" {
            return Err(anyhow!("Transaction history via Seistream API is only supported for 'sei' and 'sei-testnet' chains."));
        }
        history::get_transaction_history(&self.client, address, limit).await
    }

    pub async fn estimate_fees(
        &self,
        chain_id: &str,
        request: &EstimateFeesRequest,
    ) -> Result<EstimateFeesResponse> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        fees::estimate_fees(&self.client, rpc_url, request).await
    }

    // FIX: Centralized, secure transaction sending method
    pub async fn send_transaction(
        &self,
        chain_id: &str,
        private_key: &str,
        tx_request: TransactionRequest,
        nonce_manager: &NonceManager,
    ) -> Result<TransactionResponse> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        let wallet = wallet::import_wallet(private_key)?.private_key.parse()?;
        transactions::send_evm_transaction(rpc_url, wallet, tx_request, nonce_manager).await
    }

    // FIX: Transfer SEI tokens method
    pub async fn transfer_sei(
        &self,
        chain_id: &str,
        request: &crate::blockchain::models::SeiTransferRequest,
    ) -> Result<crate::blockchain::models::TransactionResponse> {
        let _rpc_url = self.get_rpc_url(chain_id)?;

        // Convert to TransactionRequest for EVM transaction
        let tx_request = TransactionRequest::new()
            .to(request.to_address.parse::<ethers_core::types::Address>()?)
            .value(ethers_core::types::U256::from_dec_str(&request.amount)?);

        // Use the centralized send_transaction method
        self.send_transaction(
            chain_id,
            &request.private_key,
            tx_request,
            &crate::blockchain::nonce_manager::NonceManager::new(),
        )
        .await
    }

    // FIX: New EVM-native event search
    pub async fn search_events_evm(&self, chain_id: &str, query: EventQuery) -> Result<Vec<crate::blockchain::models::SearchEventsResponse>> {
        let result = event::search_events_evm(self, chain_id, query).await?;
        Ok(vec![result])
    }
}