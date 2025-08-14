// src/mcp/wallet_storage.rs

use crate::mcp::encryption::{decrypt_private_key, encrypt_private_key};
use anyhow::{anyhow, Result, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredWallet {
    pub wallet_name: String,
    // FIX: Field name is the same, but the content will now be "salt.payload"
    pub encrypted_private_key: String,
    pub public_address: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalletStorage {
    pub wallets: HashMap<String, StoredWallet>,
    pub master_password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}


// FIX: Removed the global lazy_static WALLET_STORAGE.
// State will be managed by the main application logic.

impl WalletStorage {
    pub fn new(master_password: &str) -> Self {
        let master_password_hash = Self::hash_password(master_password);
        Self {
            wallets: HashMap::new(),
            master_password_hash,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn hash_password(password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_master_password(&self, master_password: &str) -> bool {
        self.master_password_hash == Self::hash_password(master_password)
    }

    pub fn add_wallet(
        &mut self,
        wallet_name: String,
        private_key: &str,
        public_address: String,
        master_password: &str,
    ) -> Result<()> {
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }
        if self.wallets.contains_key(&wallet_name) {
            return Err(anyhow!("Wallet with name '{}' already exists", wallet_name));
        }

        // FIX: Pass master password directly to the corrected encryption function.
        let encrypted_private_key = encrypt_private_key(private_key, master_password)?;

        let stored_wallet = StoredWallet {
            wallet_name: wallet_name.clone(),
            encrypted_private_key,
            public_address,
            created_at: Utc::now(),
        };

        self.wallets.insert(wallet_name, stored_wallet);
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn get_decrypted_private_key(
        &self,
        wallet_name: &str,
        master_password: &str,
    ) -> Result<String> {
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }
        let wallet = self
            .wallets
            .get(wallet_name)
            .ok_or_else(|| anyhow!("Wallet '{}' not found", wallet_name))?;

        // FIX: Pass master password directly to the corrected decryption function.
        decrypt_private_key(&wallet.encrypted_private_key, master_password)
    }

    pub fn list_wallets(&self) -> Vec<String> {
        self.wallets.keys().cloned().collect()
    }

    pub fn remove_wallet(&mut self, wallet_name: &str, master_password: &str) -> Result<bool> {
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }
        if self.wallets.remove(wallet_name).is_some() {
            self.updated_at = Utc::now();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}


/// Helper function to get the default path for the wallet storage file.
pub fn get_wallet_storage_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    path.push(".sei-mcp-server");
    path.push("wallets.json");
    Ok(path)
}

/// Loads a wallet storage from a file. If the file does not exist, it creates a new one.
pub fn load_or_create_wallet_storage(file_path: &Path, master_password: &str) -> Result<WalletStorage> {
    if !file_path.exists() {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let new_storage = WalletStorage::new(master_password);
        let json = serde_json::to_string_pretty(&new_storage)?;
        fs::write(file_path, json)?;
        return Ok(new_storage);
    }

    let json = fs::read_to_string(file_path).context("Failed to read wallet storage file")?;
    let storage: WalletStorage = serde_json::from_str(&json).context("Failed to parse wallet storage JSON")?;

    if !storage.verify_master_password(master_password) {
        return Err(anyhow!("Invalid master password for existing wallet storage"));
    }

    Ok(storage)
}

/// Saves the wallet storage to a file.
pub fn save_wallet_storage(file_path: &Path, storage: &WalletStorage) -> Result<()> {
    let json = serde_json::to_string_pretty(storage)?;
    fs::write(file_path, json)?;
    Ok(())
}