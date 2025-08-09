use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use crate::mcp::encryption::{decrypt_private_key, encrypt_private_key, initialize_encryption};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredWallet {
    pub wallet_name: String,
    pub encrypted_private_key: String,
    pub public_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStorage {
    pub wallets: HashMap<String, StoredWallet>,
    pub master_password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

lazy_static! {
    static ref WALLET_STORAGE: Mutex<Option<WalletStorage>> = Mutex::new(None);
}

impl WalletStorage {
    pub fn new(master_password: &str) -> Result<Self> {
        let master_password_hash = Self::hash_password(master_password);
        Ok(Self {
            wallets: HashMap::new(),
            master_password_hash,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
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
        private_key: String,
        public_address: String,
        master_password: &str,
    ) -> Result<()> {
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }

        initialize_encryption(master_password)?;
        let encrypted_private_key = encrypt_private_key(&private_key)?;

        let stored_wallet = StoredWallet {
            wallet_name: wallet_name.clone(),
            encrypted_private_key,
            public_address,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.wallets.insert(wallet_name, stored_wallet);
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn get_wallet(&self, wallet_name: &str, master_password: &str) -> Result<StoredWallet> {
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }
        self.wallets
            .get(wallet_name)
            .cloned()
            .ok_or_else(|| anyhow!("Wallet '{}' not found", wallet_name))
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

        initialize_encryption(master_password)?;
        decrypt_private_key(&wallet.encrypted_private_key)
    }

    pub fn list_wallets(&self) -> Vec<StoredWallet> {
        self.wallets.values().cloned().collect()
    }

    pub fn remove_wallet(&mut self, wallet_name: &str) -> bool {
        if self.wallets.remove(wallet_name).is_some() {
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn save_to_file(&self, file_path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, json)?;
        Ok(())
    }

    pub fn load_from_file(file_path: &Path, master_password: &str) -> Result<Self> {
        if !file_path.exists() {
            // If the file doesn't exist, create a new storage and save it immediately.
            let new_storage = Self::new(master_password)?;
            new_storage.save_to_file(file_path)?;
            return Ok(new_storage);
        }

        let json = fs::read_to_string(file_path)?;
        let storage: WalletStorage = serde_json::from_str(&json)?;

        if !storage.verify_master_password(master_password) {
            return Err(anyhow!(
                "Invalid master password for existing wallet storage"
            ));
        }

        Ok(storage)
    }
}

pub fn get_wallet_storage_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    path.push(".sei-mcp-server");
    path.push("wallets.json");
    Ok(path)
}

pub fn initialize_wallet_storage(master_password: &str) -> Result<()> {
    let storage_path = get_wallet_storage_path()?;
    let storage = WalletStorage::load_from_file(&storage_path, master_password)?;
    let mut global_storage = WALLET_STORAGE.lock().unwrap();
    *global_storage = Some(storage);
    Ok(())
}

pub fn save_wallet_storage() -> Result<()> {
    let storage_path = get_wallet_storage_path()?;
    let storage_lock = WALLET_STORAGE.lock().unwrap();
    if let Some(storage) = storage_lock.as_ref() {
        storage.save_to_file(&storage_path)?;
    }
    Ok(())
}

pub fn add_wallet_to_storage(
    wallet_name: String,
    private_key: String,
    public_address: String,
    master_password: &str,
) -> Result<()> {
    let mut storage_lock = WALLET_STORAGE.lock().unwrap();
    let storage = storage_lock
        .as_mut()
        .ok_or_else(|| anyhow!("Wallet storage not initialized"))?;
    storage.add_wallet(wallet_name, private_key, public_address, master_password)?;
    save_wallet_storage()
}

pub fn get_wallet_from_storage(wallet_name: &str, master_password: &str) -> Result<StoredWallet> {
    let storage_lock = WALLET_STORAGE.lock().unwrap();
    let storage = storage_lock
        .as_ref()
        .ok_or_else(|| anyhow!("Wallet storage not initialized"))?;
    storage.get_wallet(wallet_name, master_password)
}

pub fn get_decrypted_private_key_from_storage(
    wallet_name: &str,
    master_password: &str,
) -> Result<String> {
    let storage_lock = WALLET_STORAGE.lock().unwrap();
    let storage = storage_lock
        .as_ref()
        .ok_or_else(|| anyhow!("Wallet storage not initialized"))?;
    storage.get_decrypted_private_key(wallet_name, master_password)
}

pub fn list_wallets_from_storage() -> Result<Vec<StoredWallet>> {
    let storage_lock = WALLET_STORAGE.lock().unwrap();
    let storage = storage_lock
        .as_ref()
        .ok_or_else(|| anyhow!("Wallet storage not initialized"))?;
    Ok(storage.list_wallets())
}

pub fn remove_wallet_from_storage(wallet_name: &str) -> Result<bool> {
    let mut storage_lock = WALLET_STORAGE.lock().unwrap();
    let storage = storage_lock
        .as_mut()
        .ok_or_else(|| anyhow!("Wallet storage not initialized"))?;
    let removed = storage.remove_wallet(wallet_name);
    if removed {
        save_wallet_storage()?;
    }
    Ok(removed)
}