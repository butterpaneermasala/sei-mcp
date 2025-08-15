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
    // Kept for non-faucet tx paths
    pub tx_private_key_evm: String,
    pub default_sender_address: Option<String>,
    pub native_denom: String,
    pub native_gas_limit: u64,
    pub native_fee_amount: u64,
    pub native_chain_id: String,
    pub native_bech32_hrp: String,
}

impl Config {
    /// Loads configuration from environment variables.
    // FIX: Now returns a Result for robust error handling instead of panicking.
    pub fn from_env() -> Result<Self> {
        // Load variables from the .env file into the environment
        dotenvy::dotenv().ok();

        // Require CHAIN_RPC_URLS to be provided; no localhost fallback
        let rpc_urls_str = env::var("CHAIN_RPC_URLS").context("CHAIN_RPC_URLS must be set to a JSON map of chain_id -> RPC URL")?;
        let chain_rpc_urls: HashMap<String, String> = serde_json::from_str(&rpc_urls_str)
            .context("Invalid CHAIN_RPC_URLS JSON format")?;

        Ok(Config {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("PORT must be a valid number")?,
            chain_rpc_urls,
            websocket_url: env::var("WEBSOCKET_URL").unwrap_or_else(|_| "".to_string()),
            faucet_api_url: env::var("FAUCET_API_URL").context("FAUCET_API_URL must be set to the faucet HTTP base URL, e.g. https://your-faucet.onrender.com")?,
            // Neutral names with backward-compatible fallbacks
            tx_private_key_evm: env::var("TX_PRIVATE_KEY_EVM")
                .or_else(|_| env::var("FAUCET_PRIVATE_KEY_EVM"))
                .or_else(|_| env::var("FAUCET_PRIVATE_KEY"))
                .unwrap_or_default(),
            default_sender_address: env::var("DEFAULT_SENDER_ADDRESS").ok().or_else(|| env::var("FAUCET_ADDRESS").ok()),
            native_denom: env::var("NATIVE_DENOM").or_else(|_| env::var("FAUCET_DENOM")).unwrap_or_else(|_| "usei".to_string()),
            native_gas_limit: env::var("NATIVE_GAS_LIMIT")
                .or_else(|_| env::var("FAUCET_GAS_LIMIT"))
                .unwrap_or_else(|_| "200000".to_string())
                .parse()
                .context("NATIVE_GAS_LIMIT must be a valid number")?,
            native_fee_amount: env::var("NATIVE_FEE_AMOUNT")
                .or_else(|_| env::var("FAUCET_FEE_AMOUNT"))
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .context("NATIVE_FEE_AMOUNT must be a valid number")?,
            native_chain_id: env::var("NATIVE_CHAIN_ID").unwrap_or_else(|_| "atlantic-2".to_string()),
            native_bech32_hrp: env::var("NATIVE_BECH32_HRP").unwrap_or_else(|_| "sei".to_string()),
        })
    }
}