// src/lib.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::PathBuf;
use std::collections::HashMap;
use std::time::Instant;

// FIX: AppState is now defined in the library root (lib.rs) to be visible to all modules.
#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub sei_client: blockchain::client::SeiClient,
    pub nonce_manager: blockchain::nonce_manager::NonceManager,
    pub wallet_storage: Arc<Mutex<mcp::wallet_storage::WalletStorage>>,
    pub wallet_storage_path: Arc<PathBuf>,
    // Per-address faucet cooldowns (keyed by "{chain_id}::{address}")
    pub faucet_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,
}

pub mod api;
pub mod blockchain;
pub mod config;
pub mod mcp;