// src/config.rs

use serde::Deserialize;
use std::env;
use anyhow::{Result, anyhow};
use tracing::error;

// `#[derive(Debug, Clone, Deserialize)]` automatically implements these traits for our struct.
// `Debug` allows printing the struct for debugging.
// `Clone` is required because Axum's `with_state` method needs the state to be clonable.
// `Deserialize` is used if you were to deserialize this config from a file (e.g., JSON, TOML),
// though here we manually parse from env vars. It's good practice for consistency.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub sei_rpc_url: String,
}

impl AppConfig {
    // `from_env` attempts to load configuration values from environment variables.
    pub fn from_env() -> Result<Self> {
        // Get the PORT environment variable, defaulting to "3000" if not set.
        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        // Parse the port string into a u16, returning an error if parsing fails.
        let port = port_str.parse()
            .map_err(|e| {
                error!("Invalid PORT environment variable: {}. Defaulting to 3000.", e);
                anyhow!("Invalid PORT: {}", e)
            })?;

        // Get the SEI_RPC_URL environment variable, returning an error if not set.
        let sei_rpc_url = env::var("SEI_RPC_URL")
            .map_err(|_| {
                error!("SEI_RPC_URL environment variable not set. Please set it in your .env file or environment.");
                anyhow!("SEI_RPC_URL environment variable not set")
            })?;

        // Return the constructed AppConfig.
        Ok(Self {
            port,
            sei_rpc_url,
        })
    }
}
