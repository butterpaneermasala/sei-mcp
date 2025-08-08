use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use base64::{Engine as _, engine::general_purpose};
use rand::Rng;

pub struct EncryptionManager {
    cipher: Aes256Gcm,
}

impl EncryptionManager {
    pub fn new(master_password: &str) -> Result<Self> {
        // Derive encryption key from master password using Argon2
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(master_password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?;

        // Use the hash as the encryption key (first 32 bytes for AES-256)
        let key_bytes = general_purpose::STANDARD
            .decode(password_hash.to_string().as_bytes())
            .unwrap_or_else(|_| password_hash.to_string().as_bytes().to_vec());
        
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        let cipher = Aes256Gcm::new(key);

        Ok(Self { cipher })
    }

    pub fn encrypt_private_key(&self, private_key: &str) -> Result<String> {
        let nonce_bytes = rand::thread_rng().gen::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted = self
            .cipher
            .encrypt(nonce, private_key.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Combine nonce and encrypted data, then base64 encode
        let mut combined = Vec::new();
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&encrypted);

        Ok(general_purpose::STANDARD.encode(combined))
    }

    pub fn decrypt_private_key(&self, encrypted_data: &str) -> Result<String> {
        let combined = general_purpose::STANDARD
            .decode(encrypted_data)
            .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

        if combined.len() < 12 {
            return Err(anyhow!("Invalid encrypted data format"));
        }

        let nonce_bytes: [u8; 12] = combined[..12].try_into().unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let encrypted = &combined[12..];

        let decrypted = self
            .cipher
            .decrypt(nonce, encrypted)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(decrypted)
            .map_err(|e| anyhow!("Invalid UTF-8 in decrypted data: {}", e))
    }
}

// Global encryption manager (in production, this should be properly initialized)
lazy_static::lazy_static! {
    static ref ENCRYPTION_MANAGER: std::sync::Mutex<Option<EncryptionManager>> = std::sync::Mutex::new(None);
}

pub fn initialize_encryption(master_password: &str) -> Result<()> {
    let manager = EncryptionManager::new(master_password)?;
    let mut global_manager = ENCRYPTION_MANAGER.lock().unwrap();
    *global_manager = Some(manager);
    Ok(())
}

pub fn encrypt_private_key(private_key: &str) -> Result<String> {
    let manager = ENCRYPTION_MANAGER.lock().unwrap();
    let manager = manager.as_ref()
        .ok_or_else(|| anyhow!("Encryption manager not initialized. Call initialize_encryption first."))?;
    manager.encrypt_private_key(private_key)
}

pub fn decrypt_private_key(encrypted_private_key: &str) -> Result<String> {
    let manager = ENCRYPTION_MANAGER.lock().unwrap();
    let manager = manager.as_ref()
        .ok_or_else(|| anyhow!("Encryption manager not initialized. Call initialize_encryption first."))?;
    manager.decrypt_private_key(encrypted_private_key)
} 