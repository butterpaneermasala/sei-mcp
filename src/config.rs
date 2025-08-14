// src/config.rs

use std::collections::HashMap;
use std::env;

// A struct to hold all configuration, loaded once at startup from the .env file.
#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub chain_rpc_urls: HashMap<String, String>,
    pub websocket_url: String,
    pub faucet_private_key: String,
    pub faucet_amount_usei: u64,
    pub faucet_denom: String,
    pub faucet_prefix: String,
    pub faucet_gas_limit: u64,
    pub faucet_fee_amount: u64,
}

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, env::VarError> {
        // Load variables from the .env file into the environment
        dotenvy::dotenv().ok();

        let rpc_urls_str = env::var("CHAIN_RPC_URLS").unwrap_or_else(|_| "{}".to_string());
        let chain_rpc_urls: HashMap<String, String> = serde_json::from_str(&rpc_urls_str)
            .expect("CHAIN_RPC_URLS must be a valid JSON string of chain_id:rpc_url pairs");

        Ok(Config {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a valid number"),
            chain_rpc_urls,
            websocket_url: env::var("WEBSOCKET_URL").unwrap_or_else(|_| "".to_string()),
            faucet_private_key: env::var("FAUCET_PRIVATE_KEY")?,
            faucet_amount_usei: env::var("FAUCET_AMOUNT_USEI")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()
                .expect("FAUCET_AMOUNT_USEI must be a valid number"),
            faucet_denom: env::var("FAUCET_DENOM").unwrap_or_else(|_| "usei".to_string()),
            faucet_prefix: env::var("FAUCET_PREFIX").unwrap_or_else(|_| "sei".to_string()),
            faucet_gas_limit: env::var("FAUCET_GAS_LIMIT")
                .unwrap_or_else(|_| "200000".to_string())
                .parse()
                .expect("FAUCET_GAS_LIMIT must be a valid number"),
            faucet_fee_amount: env::var("FAUCET_FEE_AMOUNT")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .expect("FAUCET_FEE_AMOUNT must be a valid number"),
        })
    }
}