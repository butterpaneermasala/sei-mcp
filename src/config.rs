use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use tracing::error;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub chain_rpc_urls: HashMap<String, String>,
    pub websocket_url: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        let port = port_str.parse().map_err(|e| {
            error!(
                "Invalid PORT environment variable: {}. Defaulting to 3000.",
                e
            );
            anyhow!("Invalid PORT: {}", e)
        })?;

        // Expect SEI_RPC_URL and other chain URLs in a comma-separated list
        // e.g., CHAIN_RPC_URLS="sei=https://rpc.sei.io,ethereum=https://mainnet.infura.io/v3/..."
        let rpc_urls_str = env::var("CHAIN_RPC_URLS").map_err(|_| {
            error!("CHAIN_RPC_URLS environment variable not set. Please set it.");
            anyhow!("CHAIN_RPC_URLS environment variable not set")
        })?;

        let mut chain_rpc_urls = HashMap::new();
        for entry in rpc_urls_str.split(',') {
            let parts: Vec<&str> = entry.splitn(2, '=').collect();
            if parts.len() == 2 {
                chain_rpc_urls.insert(parts[0].to_string(), parts[1].to_string());
            } else {
                error!("Invalid format for CHAIN_RPC_URLS entry: {}", entry);
            }
        }

        if chain_rpc_urls.is_empty() {
            return Err(anyhow!("No valid RPC URLs found in CHAIN_RPC_URLS"));
        }

        // Get websocket URL from environment variable
        let websocket_url =
            env::var("WEBSOCKET_URL").unwrap_or_else(|_| "wss://rpc.sei.io/websocket".to_string());

        Ok(Self {
            port,
            chain_rpc_urls,
            websocket_url,
        })
    }
}
