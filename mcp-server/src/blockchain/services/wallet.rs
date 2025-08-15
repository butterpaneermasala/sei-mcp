use anyhow::Result;
use bip39::{Language, Mnemonic};
use k256::ecdsa::SigningKey;
use ethers_core::types::Address;
use ethers_core::utils::{hex, keccak256};
use rand::RngCore;
use std::str::FromStr;
use tracing::info;
use crate::blockchain::models::{ChainType, DualNetworkWallet, ImportWalletError, WalletResponse, WalletGenerationError};
use secrecy::{Secret, SecretString};
use bip32::{DerivationPath, XPrv};

// Network-specific derivation paths
const SEI_NATIVE_HD_PATH: &str = "m/44'/118'/0'/0/0"; // Cosmos path
const SEI_EVM_HD_PATH: &str = "m/44'/60'/0'/0/0";    // Ethereum path

/// Enhanced wallet generation with network-aware security
#[derive(Debug, Clone)]
pub struct SecureWalletManager {
    chain_type: ChainType,
}

impl SecureWalletManager {
    pub fn new(chain_type: ChainType) -> Self {
        Self { chain_type }
    }

    /// Generate a secure wallet for the specified network
    pub fn generate_wallet(&self) -> Result<WalletResponse, WalletGenerationError> {
        info!("Generating secure wallet for {:?} network", self.chain_type);
        
        let mut entropy = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        let phrase = mnemonic.to_string();

        let seed = mnemonic.to_seed("");
        let private_key = self.derive_network_key(&seed)?;
        
        let mut dual_wallet = DualNetworkWallet::from_private_key(&private_key.to_bytes());
        dual_wallet.mnemonic = Some(SecretString::new(phrase.clone()));
        Ok(WalletResponse {
            address: dual_wallet.address_for_network(self.chain_type),
            private_key: dual_wallet.private_key_hex(),
            mnemonic: dual_wallet.mnemonic_string(),
        })
    }

    /// Import wallet with network-specific validation
    pub fn import_wallet(&self, input: &str) -> Result<WalletResponse, ImportWalletError> {
        info!("Importing wallet for {:?} network", self.chain_type);
        
        if let Ok(mnemonic) = Mnemonic::from_str(input) {
            let seed = mnemonic.to_seed("");
            let private_key = self.derive_network_key(&seed)
                .map_err(|e| ImportWalletError::InvalidMnemonic(e.to_string()))?;
            
            let mut dual_wallet = DualNetworkWallet::from_private_key(&private_key.to_bytes());
            dual_wallet.mnemonic = Some(SecretString::new(mnemonic.to_string()));
            Ok(WalletResponse {
                address: dual_wallet.address_for_network(self.chain_type),
                private_key: dual_wallet.private_key_hex(),
                mnemonic: dual_wallet.mnemonic_string(),
            })
        } else if input.starts_with("0x") && (input.len() == 66 || input.len() == 64) {
            let key_str = input.strip_prefix("0x").unwrap_or(input);
            let private_key_bytes = hex::decode(key_str)
                .map_err(|e| ImportWalletError::InvalidPrivateKey(format!("Invalid hex: {}", e)))?;
                
            let dual_wallet = DualNetworkWallet::from_private_key(&private_key_bytes);
            Ok(WalletResponse {
                address: dual_wallet.address_for_network(self.chain_type),
                private_key: dual_wallet.private_key_hex(),
                mnemonic: None,
            })
        } else {
            Err(ImportWalletError::InvalidInput(
                "Input must be a valid mnemonic phrase or private key (with or without 0x prefix)".to_string(),
            ))
        }
    }

    /// Derive network-specific private key using BIP44 (secp256k1)
    fn derive_network_key(&self, seed_bytes: &[u8]) -> Result<SigningKey, WalletGenerationError> {
        // Choose derivation path based on network
        let path_str = match self.chain_type {
            ChainType::Native => SEI_NATIVE_HD_PATH,
            ChainType::Evm => SEI_EVM_HD_PATH,
        };

        let derivation_path: DerivationPath = path_str
            .parse()
            .map_err(|e| WalletGenerationError::KeyGenerationFailed(format!("Invalid derivation path {}: {}", path_str, e)))?;

        // Derive child extended private key directly from seed and path
        let child = XPrv::derive_from_path(seed_bytes, &derivation_path)
            .map_err(|e| WalletGenerationError::KeyGenerationFailed(format!("derive_from_path failed: {}", e)))?;

        // Obtain raw 32-byte secret and turn into k256 SigningKey
        let secret_bytes = child.private_key().to_bytes();
        SigningKey::from_slice(&secret_bytes)
            .map_err(|e| WalletGenerationError::KeyGenerationFailed(format!("Failed to create signing key: {}", e)))
    }

    /// Validate address format for the network
    pub fn validate_address(&self, address: &str) -> Result<bool> {
        match self.chain_type {
            ChainType::Native => {
                // Validate bech32 format for Sei native (sei1...)
                Ok(address.starts_with("sei1") && address.len() >= 39)
            },
            ChainType::Evm => {
                // Validate Ethereum hex format (0x...)
                Ok(address.starts_with("0x") && address.len() == 42 && 
                   hex::decode(&address[2..]).is_ok())
            }
        }
    }
}

// DualNetworkWallet implementation moved to models.rs to avoid duplication

impl DualNetworkWallet {
    /// Generate both EVM and native Sei addresses from the same private key
    pub fn from_private_key(private_key_bytes: &[u8]) -> Self {
        let signing_key = SigningKey::from_slice(private_key_bytes).expect("Valid private key");
        let public_key = signing_key.verifying_key();
        let encoded_point = public_key.to_encoded_point(false);
        let pubkey_bytes = encoded_point.as_bytes();
        
        // Generate EVM address (Ethereum-style)
        let hash = keccak256(&pubkey_bytes[1..]);
        let evm_address = Address::from_slice(&hash[12..]);
        let evm_address_hex = format!("0x{}", hex::encode(evm_address));
        
        // Generate native Sei address (bech32-style)
        let native_address = Self::generate_sei_native_address(&pubkey_bytes[1..]);
        
        // Store private key securely
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&private_key_bytes[0..32]);
        let private_key_secret: Secret<[u8; 32]> = Secret::new(arr);
        
        Self {
            evm_address: evm_address_hex,
            native_address,
            private_key: private_key_secret,
            mnemonic: None,
        }
    }

    /// Generate a proper Sei native address using bech32 encoding
    fn generate_sei_native_address(pubkey_bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        use ripemd::Ripemd160;
        use bech32::{ToBase32, Variant};
        
        // Cosmos-style address derivation: SHA256 -> RIPEMD160
        let sha_hash = Sha256::digest(pubkey_bytes);
        let ripemd_hash = Ripemd160::digest(sha_hash);
        
        // Proper bech32 encoding with HRP "sei"
        match bech32::encode("sei", ripemd_hash.to_base32(), Variant::Bech32) {
            Ok(addr) => addr,
            // Fallback to previous simplified form on any error
            Err(_) => format!("sei1{}", hex::encode(&ripemd_hash[..20])),
        }
    }
}

pub fn create_wallet() -> Result<WalletResponse, WalletGenerationError> {
    // Default to EVM for backward compatibility
    let manager = SecureWalletManager::new(ChainType::Evm);
    manager.generate_wallet()
}

pub fn create_wallet_for_network(chain_type: ChainType) -> Result<WalletResponse, WalletGenerationError> {
    let manager = SecureWalletManager::new(chain_type);
    manager.generate_wallet()
}

pub fn import_wallet(input: &str) -> Result<WalletResponse, ImportWalletError> {
    // Default to EVM for backward compatibility
    let manager = SecureWalletManager::new(ChainType::Evm);
    manager.import_wallet(input)
}

pub fn import_wallet_for_network(chain_type: ChainType, input: &str) -> Result<WalletResponse, ImportWalletError> {
    let manager = SecureWalletManager::new(chain_type);
    manager.import_wallet(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::{Mnemonic, Language};

    #[test]
    fn test_bip44_derivation_native_address_format() {
        // Deterministic mnemonic for test
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::parse_in(Language::English, phrase).unwrap();
        let manager = SecureWalletManager::new(ChainType::Native);
        let seed = mnemonic.to_seed("");
        let sk = manager.derive_network_key(&seed).expect("derive native sk");
        let wallet = DualNetworkWallet::from_private_key(&sk.to_bytes());
        let addr = wallet.address_for_network(ChainType::Native);
        assert!(addr.starts_with("sei1"), "native address should start with sei1: {}", addr);
        assert!(addr.len() >= 39, "native bech32 length looks short: {}", addr);
    }

    #[test]
    fn test_bip44_derivation_evm_address_format() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::parse_in(Language::English, phrase).unwrap();
        let manager = SecureWalletManager::new(ChainType::Evm);
        let seed = mnemonic.to_seed("");
        let sk = manager.derive_network_key(&seed).expect("derive evm sk");
        let wallet = DualNetworkWallet::from_private_key(&sk.to_bytes());
        let addr = wallet.address_for_network(ChainType::Evm);
        assert!(addr.starts_with("0x") && addr.len() == 42, "evm address invalid: {}", addr);
    }
}
