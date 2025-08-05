// src/blockchain/models.rs
use serde::{Deserialize, Serialize};
use thiserror::Error;

// --- Error types for wallet operations ---

#[derive(Error, Debug)]
pub enum WalletGenerationError {
    #[error("failed to generate mnemonic: {0}")]
    MnemonicError(#[from] bip39::Error),
    #[error("failed to derive wallet from mnemonic: {0}")]
    DerivationError(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum ImportWalletError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),
}

// --- Wallet Models ---

/// Defines the structure for a generated or imported wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletResponse {
    pub address: String,
    pub private_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
}

/// Defines the structure for the request to import a wallet.
#[derive(Debug, Deserialize)]
pub struct ImportWalletRequest {
    pub mnemonic_or_private_key: String,
}

// --- Balance Models ---

/// Defines the structure for a balance response from the blockchain client.
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub amount: String,
    pub denom: String,
}

// --- Transaction History Models ---

/// Defines the structure for a single transaction (our internal representation).
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub denom: String,
    pub timestamp: String,
}

/// Defines the structure for the transaction history response.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionHistoryResponse {
    pub transactions: Vec<Transaction>,
}

// --- Fee Estimation Models ---

/// Defines the structure for a fee estimation request.
#[derive(Debug, Serialize, Deserialize)]
pub struct EstimateFeesRequest {
    pub from: String,
    pub to: String,
    pub amount: String,
}

/// Defines the structure for a fee estimation response.
#[derive(Debug, Serialize)]
pub struct EstimateFeesResponse {
    pub estimated_gas: String,
    pub gas_price: String,
    pub total_fee: String,
    pub denom: String,
}
