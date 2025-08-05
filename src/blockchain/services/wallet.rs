use anyhow::{Result, anyhow};
use bip39::{Language, Mnemonic};
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::types::Address;
use ethers_core::utils::{hex, keccak256};
use rand::RngCore;
use std::str::FromStr;
use tracing::info;
use crate::blockchain::models::{WalletResponse, WalletGenerationError, ImportWalletError};

pub fn create_wallet() -> Result<WalletResponse, WalletGenerationError> {
    info!("Generating a new wallet...");
    let mut entropy = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
    let phrase = mnemonic.to_string();

    let seed = mnemonic.to_seed("");
    let private_key = SigningKey::from_slice(&seed[..32])
        .map_err(|e| anyhow!("Failed to create signing key from seed: {}", e))?;

    let public_key = private_key.verifying_key();
    let encoded_point = public_key.to_encoded_point(false);
    let pubkey_bytes = encoded_point.as_bytes();
    let hash = keccak256(&pubkey_bytes[1..]);
    let address = Address::from_slice(&hash[12..]);

    let private_key_hex = hex::encode(private_key.to_bytes());
    let address_hex = format!("0x{}", hex::encode(address));

    info!("Wallet generated successfully.");
    Ok(WalletResponse {
        address: address_hex,
        private_key: private_key_hex,
        mnemonic: Some(phrase),
    })
}

pub fn import_wallet(input: &str) -> Result<WalletResponse, ImportWalletError> {
    info!("Attempting to import a wallet...");

    if let Ok(mnemonic) = Mnemonic::from_str(input) {
        let seed = mnemonic.to_seed("");
        let private_key = SigningKey::from_slice(&seed[..32]).map_err(|e| {
            ImportWalletError::InvalidMnemonic(format!(
                "Failed to create signing key from mnemonic: {}",
                e
            ))
        })?;

        let public_key = private_key.verifying_key();
        let encoded_point = public_key.to_encoded_point(false);
        let pubkey_bytes = encoded_point.as_bytes();
        let hash = keccak256(&pubkey_bytes[1..]);
        let address = Address::from_slice(&hash[12..]);

        let private_key_hex = hex::encode(private_key.to_bytes());
        let address_hex = format!("0x{}", hex::encode(address));

        info!("Wallet imported successfully from mnemonic.");
        return Ok(WalletResponse {
            address: address_hex,
            private_key: private_key_hex,
            mnemonic: Some(input.to_string()),
        });
    }

    if let Ok(private_key_bytes) = hex::decode(input.trim_start_matches("0x")) {
        let private_key = SigningKey::from_slice(&private_key_bytes).map_err(|e| {
            ImportWalletError::InvalidPrivateKey(format!(
                "Failed to create signing key from private key: {}",
                e
            ))
        })?;

        let public_key = private_key.verifying_key();
        let encoded_point = public_key.to_encoded_point(false);
        let pubkey_bytes = encoded_point.as_bytes();
        let hash = keccak256(&pubkey_bytes[1..]);
        let address = Address::from_slice(&hash[12..]);

        let private_key_hex = hex::encode(private_key.to_bytes());
        let address_hex = format!("0x{}", hex::encode(address));

        info!("Wallet imported successfully from private key.");
        return Ok(WalletResponse {
            address: address_hex,
            private_key: private_key_hex,
            mnemonic: None,
        });
    }

    Err(ImportWalletError::InvalidPrivateKey(
        "Input is not a valid mnemonic or private key".to_string(),
    ))
}
