// src/lib.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::PathBuf;

// FIX: AppState is now defined in the library root (lib.rs) to be visible to all modules.

// Re-export utils module
pub mod utils;
#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub sei_client: blockchain::client::SeiClient,
    pub nonce_manager: blockchain::nonce_manager::NonceManager,
    pub wallet_storage: Arc<Mutex<mcp::wallet_storage::WalletStorage>>,
    pub wallet_storage_path: Arc<PathBuf>,
}

pub mod api;
pub mod blockchain;
pub mod config;
pub mod mcp;