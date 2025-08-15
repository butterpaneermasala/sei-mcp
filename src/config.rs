// src/config.rs

use std::collections::HashMap;
use std::env;
use anyhow::{Context, Result};

// A struct to hold all configuration, loaded once at startup from the .env file.
#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub chain_rpc_urls: HashMap<String, String>,
    pub websocket_url: String,
    pub faucet_api_url: String,
    pub faucet_private_key: String,
    pub faucet_private_key_evm: String,
    pub faucet_private_key_native: String,
    pub faucet_address: Option<String>, // Optional: only needed for legacy native send
    pub faucet_amount_usei: u64,
    pub faucet_denom: String,
    pub faucet_gas_limit: u64,
    pub faucet_fee_amount: u64,
    // Native (Cosmos) chain params for signing
    pub native_chain_id: String,
    pub native_bech32_hrp: String,
    // --- New: Rate limiting & cooldown config (operator-only) ---
    pub tx_rate_window_secs: u64,
    pub tx_rate_max: usize,
    pub faucet_rate_window_secs: u64,
    pub faucet_rate_max: usize,
    pub faucet_address_cooldown_secs: u64,
}

impl Config {
    /// Loads configuration from environment variables.
    // FIX: Now returns a Result for robust error handling instead of panicking.
    pub fn from_env() -> Result<Self> {
        // Load variables from the .env file into the environment
        dotenvy::dotenv().ok();

        // Use default RPC URLs if not provided or if parsing fails
        let rpc_urls_str = env::var("CHAIN_RPC_URLS").unwrap_or_else(|_| r#"{"localhost":"http://127.0.0.1:8545"}"#.to_string());
        let chain_rpc_urls: HashMap<String, String> = serde_json::from_str(&rpc_urls_str)
            .unwrap_or_else(|_| {
                eprintln!("Warning: Invalid CHAIN_RPC_URLS format, using defaults. Got: '{}'", rpc_urls_str);
                let mut default_urls = HashMap::new();
                default_urls.insert("localhost".to_string(), "http://127.0.0.1:8545".to_string());
                default_urls.insert("sei-testnet".to_string(), "https://evm-rpc-testnet.sei-apis.com".to_string());
                default_urls
            });

        // Read faucet keys into locals so we can validate/log
        let faucet_private_key = env::var("FAUCET_PRIVATE_KEY").unwrap_or_default();
        let faucet_private_key_evm = env::var("FAUCET_PRIVATE_KEY_EVM")
            .unwrap_or_else(|_| env::var("FAUCET_PRIVATE_KEY").unwrap_or_default());
        let faucet_private_key_native = env::var("FAUCET_PRIVATE_KEY_NATIVE")
            .unwrap_or_else(|_| env::var("FAUCET_PRIVATE_KEY").unwrap_or_default());

        if faucet_private_key_evm.trim().is_empty() {
            tracing::warn!(
                "FAUCET_PRIVATE_KEY_EVM is empty; EVM faucet requests will fail until it is set"
            );
        }

        Ok(Config {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("PORT must be a valid number")?,
            chain_rpc_urls,
            websocket_url: env::var("WEBSOCKET_URL").unwrap_or_else(|_| "".to_string()),
            faucet_api_url: env::var("FAUCET_API_URL").context("FAUCET_API_URL must be set to the faucet HTTP base URL, e.g. https://your-faucet.onrender.com")?,
            // Optional: legacy/global faucet key. Prefer network-specific keys below.
            faucet_private_key,
            faucet_private_key_evm,
            faucet_private_key_native,
            faucet_address: env::var("FAUCET_ADDRESS").ok(),
            faucet_amount_usei: env::var("FAUCET_AMOUNT_USEI")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()
                .context("FAUCET_AMOUNT_USEI must be a valid number")?,
            faucet_denom: env::var("FAUCET_DENOM").unwrap_or_else(|_| "usei".to_string()),
            // FIX: Removed faucet_prefix as it's for native Cosmos addresses.
            faucet_gas_limit: env::var("FAUCET_GAS_LIMIT")
                .unwrap_or_else(|_| "200000".to_string())
                .parse()
                .context("FAUCET_GAS_LIMIT must be a valid number")?,
            faucet_fee_amount: env::var("FAUCET_FEE_AMOUNT")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .context("FAUCET_FEE_AMOUNT must be a valid number")?,
            native_chain_id: env::var("NATIVE_CHAIN_ID").unwrap_or_else(|_| "atlantic-2".to_string()),
            native_bech32_hrp: env::var("NATIVE_BECH32_HRP").unwrap_or_else(|_| "sei".to_string()),
            // --- New: Rate limiting & cooldown config with defaults ---
            tx_rate_window_secs: env::var("TX_RATE_WINDOW_SECS").unwrap_or_else(|_| "60".to_string()).parse().context("TX_RATE_WINDOW_SECS must be a valid number")?,
            tx_rate_max: env::var("TX_RATE_MAX").unwrap_or_else(|_| "10".to_string()).parse().context("TX_RATE_MAX must be a valid number")?,
            faucet_rate_window_secs: env::var("FAUCET_RATE_WINDOW_SECS").unwrap_or_else(|_| "60".to_string()).parse().context("FAUCET_RATE_WINDOW_SECS must be a valid number")?,
            faucet_rate_max: env::var("FAUCET_RATE_MAX").unwrap_or_else(|_| "2".to_string()).parse().context("FAUCET_RATE_MAX must be a valid number")?,
            faucet_address_cooldown_secs: env::var("FAUCET_ADDRESS_COOLDOWN_SECS").unwrap_or_else(|_| "86400".to_string()).parse().context("FAUCET_ADDRESS_COOLDOWN_SECS must be a valid number")?,
        })
    }
}