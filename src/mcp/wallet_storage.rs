use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};
use crate::mcp::encryption::{encrypt_private_key, decrypt_private_key, initialize_encryption};

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
    pub master_password_hash: String, // Hash of the master password for verification
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
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_master_password(&self, master_password: &str) -> bool {
        self.master_password_hash == Self::hash_password(master_password)
    }

    pub fn add_wallet(&mut self, wallet_name: String, private_key: String, public_address: String, master_password: &str) -> Result<()> {
        // Verify master password
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }

        // Initialize encryption with master password
        initialize_encryption(master_password)?;

        // Encrypt the private key
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
        // Verify master password
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }

        let wallet = self.wallets.get(wallet_name)
            .ok_or_else(|| anyhow!("Wallet '{}' not found", wallet_name))?;

        Ok(wallet.clone())
    }

    pub fn get_decrypted_private_key(&self, wallet_name: &str, master_password: &str) -> Result<String> {
        // Verify master password
        if !self.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password"));
        }

        let wallet = self.wallets.get(wallet_name)
            .ok_or_else(|| anyhow!("Wallet '{}' not found", wallet_name))?;

        // Initialize encryption with master password
        initialize_encryption(master_password)?;

        // Decrypt the private key
        decrypt_private_key(&wallet.encrypted_private_key)
    }

    pub fn list_wallets(&self) -> Vec<StoredWallet> {
        self.wallets.values().cloned().collect()
    }

    pub fn remove_wallet(&mut self, wallet_name: &str) -> bool {
        let removed = self.wallets.remove(wallet_name).is_some();
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }

    pub fn save_to_file(&self, file_path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| anyhow!("Failed to serialize wallet storage: {}", e))?;
        
        fs::write(file_path, json)
            .map_err(|e| anyhow!("Failed to write wallet storage file: {}", e))?;
        
        Ok(())
    }

    pub fn load_from_file(file_path: &Path, master_password: &str) -> Result<Self> {
        if !file_path.exists() {
            return Self::new(master_password);
        }

        let json = fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read wallet storage file: {}", e))?;
        
        let storage: WalletStorage = serde_json::from_str(&json)
            .map_err(|e| anyhow!("Failed to parse wallet storage file: {}", e))?;

        // Verify master password
        if !storage.verify_master_password(master_password) {
            return Err(anyhow!("Invalid master password for existing wallet storage"));
        }

        Ok(storage)
    }
}

pub fn get_wallet_storage_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".sei-mcp-server");
    path.push("wallets.json");
    path
}

pub fn initialize_wallet_storage(master_password: &str) -> Result<()> {
    let storage_path = get_wallet_storage_path();
    
    // Create directory if it doesn't exist
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create wallet storage directory: {}", e))?;
    }

    let storage = WalletStorage::load_from_file(&storage_path, master_password)?;
    
    let mut global_storage = WALLET_STORAGE.lock().unwrap();
    *global_storage = Some(storage);
    
    Ok(())
}

pub fn save_wallet_storage() -> Result<()> {
    let storage_path = get_wallet_storage_path();
    let storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_ref() {
        storage.save_to_file(&storage_path)?;
    }
    
    Ok(())
}

pub fn add_wallet_to_storage(wallet_name: String, private_key: String, public_address: String, master_password: &str) -> Result<()> {
    let mut storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_mut() {
        storage.add_wallet(wallet_name, private_key, public_address, master_password)?;
        // Save to disk immediately
        save_wallet_storage()?;
    } else {
        return Err(anyhow!("Wallet storage not initialized"));
    }
    
    Ok(())
}

pub fn get_wallet_from_storage(wallet_name: &str, master_password: &str) -> Result<StoredWallet> {
    let storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_ref() {
        storage.get_wallet(wallet_name, master_password)
    } else {
        Err(anyhow!("Wallet storage not initialized"))
    }
}

pub fn get_decrypted_private_key_from_storage(wallet_name: &str, master_password: &str) -> Result<String> {
    let storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_ref() {
        storage.get_decrypted_private_key(wallet_name, master_password)
    } else {
        Err(anyhow!("Wallet storage not initialized"))
    }
}

pub fn list_wallets_from_storage() -> Result<Vec<StoredWallet>> {
    let storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_ref() {
        Ok(storage.list_wallets())
    } else {
        Err(anyhow!("Wallet storage not initialized"))
    }
}

pub fn remove_wallet_from_storage(wallet_name: &str) -> Result<bool> {
    let mut storage = WALLET_STORAGE.lock().unwrap();
    
    if let Some(storage) = storage.as_mut() {
        let removed = storage.remove_wallet(wallet_name);
        if removed {
            // Save to disk immediately
            save_wallet_storage()?;
        }
        Ok(removed)
    } else {
        Err(anyhow!("Wallet storage not initialized"))
    }
} 