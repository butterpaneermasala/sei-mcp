// src/blockchain/models.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// --- Error types for wallet operations ---

#[derive(Error, Debug)]
pub enum WalletGenerationError {
    #[error("failed to generate mnemonic: {0}")]
    MnemonicError(#[from] bip39::Error),
    #[error("failed to derive wallet from mnemonic: {0}")]
    DerivationError(#[from] anyhow::Error),
    #[error("key generation failed: {0}")]
    KeyGenerationFailed(String),
}

#[derive(Error, Debug)]
pub enum ImportWalletError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

#[derive(Error, Debug)]
pub enum CreateWalletError {
    #[error("failed to generate wallet: {0}")]
    GenerationFailed(String),
    #[error("key derivation failed: {0}")]
    KeyDerivationFailed(String),
}

// --- Wallet Models ---

/// Dual network wallet containing both EVM and native addresses
#[derive(Debug, Clone)]
pub struct DualNetworkWallet {
    pub evm_address: String,
    pub native_address: String,
    // Store sensitive data securely in memory
    pub private_key: secrecy::Secret<[u8; 32]>,
    pub mnemonic: Option<secrecy::SecretString>,
}

impl DualNetworkWallet {
    pub fn to_wallet_response(&self) -> WalletResponse {
        WalletResponse {
            address: self.evm_address.clone(), // Default to EVM for backward compatibility
            private_key: self.private_key_hex(),
            mnemonic: self.mnemonic_string(),
        }
    }

    /// Return hex-encoded private key for API response (avoid logging elsewhere)
    pub fn private_key_hex(&self) -> String {
        use secrecy::ExposeSecret;
        let bytes = self.private_key.expose_secret();
        hex::encode(bytes)
    }

    /// Return mnemonic as String for API response if present
    pub fn mnemonic_string(&self) -> Option<String> {
        use secrecy::ExposeSecret;
        self.mnemonic.as_ref().map(|s| s.expose_secret().clone())
    }

    /// Get the appropriate address for the specified network
    pub fn address_for_network(&self, chain_type: ChainType) -> String {
        match chain_type {
            ChainType::Evm => self.evm_address.clone(),
            ChainType::Native => self.native_address.clone(),
        }
    }
}

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

/// Enum to distinguish between native and token transfers.
#[derive(Debug, Serialize, Deserialize)]
pub enum TransactionType {
    Native,
    ERC20,
}

/// Defines the structure for a single transaction (our internal representation).
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub denom: String, // 'usei' for native, token symbol for ERC20
    pub timestamp: String,
    pub transaction_type: TransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<String>,
}

/// Defines the structure for the transaction history response.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionHistoryResponse {
    pub transactions: Vec<Transaction>,
}

// --- Transfer Models ---

/// Defines the structure for a SEI token transfer request.
#[derive(Debug, Serialize, Deserialize)]
pub struct SeiTransferRequest {
    pub to_address: String,
    pub amount: String,
    pub private_key: String,
    pub gas_limit: Option<String>,
    pub gas_price: Option<String>,
}

/// Defines the structure for a token transfer request.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenTransferRequest {
    pub to_address: String,
    pub contract_address: String,
    pub amount: String,
    pub private_key: String,
}

/// Defines the structure for an NFT transfer request.
#[derive(Debug, Serialize, Deserialize)]
pub struct NftTransferRequest {
    pub to_address: String,
    pub contract_address: String,
    pub token_id: String,
    pub private_key: String,
}

/// Defines the structure for a token approval request.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveRequest {
    pub spender_address: String,
    pub contract_address: String,
    pub amount: String,
    pub private_key: String,
}

/// Defines the structure for a transaction response.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub tx_hash: String,
}

/// Defines the structure for token information response.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenInfoResponse {
    pub name: String,
    pub symbol: String,
    pub decimals: u64,
    pub contract_address: String,
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

/// Represents the query parameters for searching events.
#[derive(Debug, Clone)]
pub struct EventQuery {
    pub contract_address: Option<String>,
    pub event_type: Option<String>,
    pub attribute_key: Option<String>,
    pub attribute_value: Option<String>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

/// The response structure for the search_events endpoint.
#[derive(Serialize, Deserialize, Debug)]
pub struct SearchEventsResponse {
    pub txs: Vec<serde_json::Value>,
    pub total_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainType {
    Native,
    Evm,
}

impl ChainType {
    pub fn from_chain_id(chain_id: &str) -> Self {
        // You can refine this logic as needed for your deployment
        if chain_id.contains("evm") || chain_id.starts_with("0x") {
            ChainType::Evm
        } else {
            ChainType::Native
        }
    }
}
// src/blockchain/models.rs

// ... (existing structs)

// --- Contract Models ---

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub hash: String,
    pub balance: String,
    pub name: String,
    pub creator_address: Option<String>,
    pub tx_hash: Option<String>,
    pub compiler_version: String,
    pub evm_version: String,
    pub optimization: bool,
    pub optimization_runs: String,
    pub code_checked_at: Option<String>,
    pub pointer_type: String,
    pub pointee_address: String,
    pub pointer_address: String,
    pub is_base_asset: bool,
    pub is_pointer: bool,
    pub proxy_type: Option<String>,
    pub implementations: Option<Vec<String>>,
    pub partially_verified: bool,
    pub fully_verified: bool,
    pub verified: bool,
    pub token: Option<TokenInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    #[serde(rename = "type")]
    pub token_type: String,
    pub token: TokenDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenDetails {
    pub hash: String,
    pub name: String,
    pub symbol: String,
    pub decimals: String,
    pub total_supply: String,
    pub id: String,
    pub address: String,
    pub pointer_type: String,
    pub pointee_address: String,
    pub pointer_address: String,
    pub is_base_asset: bool,
    pub is_pointer: bool,
    pub holders: u64,
    pub transfers: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractCode {
    pub abi: Vec<Value>,
    pub compiler_settings: Value,
    pub external_libraries: Vec<Value>,
    pub runtime_code: String,
    pub creation_code: String,
    pub sources: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContractTransactionsResponse {
    pub items: Vec<ContractTransaction>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractTransaction {
    pub hash: String,
    pub timestamp: String,
    pub value: String,
    pub fee: String,
    #[serde(rename = "type")]
    pub tx_type: u64,
    pub action_type: String,
    pub gas_price: String,
    pub gas_limit: String,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub priority_fee: String,
    pub burnt_fees: String,
    pub gas_used_by_transaction: String,
    pub nonce: u64,
    pub status: bool,
    pub failure_reason: Option<String>,
    pub height: u64,
    pub to: String,
    pub from: String,
    pub data: String,
    pub method: String,
    pub block_confirmation: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub pages: u64,
    pub rows: u64,
    pub curr_page: u64,
    pub next_page: Option<u64>,
}
