// src/blockchain/services/faucet.rs

use crate::blockchain::models::ChainType;
use crate::blockchain::services::transactions::{send_evm_transaction, send_native_transaction_signed};
use crate::config::Config;
use ethers_core::types::{Address, TransactionRequest, U256};
use ethers_signers::LocalWallet;
use anyhow::{Result, Context};
use std::str::FromStr;
use tracing::info;

/// Sends faucet tokens via a standard EVM transaction.
pub async fn send_faucet_tokens(
    config: &Config,
    recipient_address: &str,
    nonce_manager: &crate::blockchain::nonce_manager::NonceManager,
    rpc_url: &str,
    chain_id: &str,
) -> Result<String> {
    let chain_type = ChainType::from_chain_id(chain_id);

    match chain_type {
        ChainType::Evm => {
            info!("Initiating EVM faucet transfer to {}", recipient_address);
            if config.faucet_private_key_evm.trim().is_empty() {
                anyhow::bail!("FAUCET_PRIVATE_KEY_EVM not set; cannot send EVM faucet transactions");
            }
            let wallet = LocalWallet::from_str(&config.faucet_private_key_evm)
                .context("Failed to load faucet wallet from private key")?;
            let recipient = Address::from_str(recipient_address)
                .context("Invalid recipient EVM address format")?;
            let value = U256::from(config.faucet_amount_usei);
            let gas_limit = U256::from(config.faucet_gas_limit);
            let gas_price = U256::from(config.faucet_fee_amount);

            let tx_request = TransactionRequest::new()
                .to(recipient)
                .value(value)
                .gas(gas_limit)
                .gas_price(gas_price);

            let tx_response = send_evm_transaction(
                rpc_url,
                wallet,
                tx_request,
                nonce_manager
            ).await?;
            Ok(tx_response.tx_hash)
        }
        ChainType::Native => {
            info!("Initiating native SEI faucet transfer to {}", recipient_address);
            let tx_hash = send_native_transaction_signed(
                config,
                rpc_url,
                &config.faucet_private_key_native,
                recipient_address,
                config.faucet_amount_usei,
            ).await?;
            Ok(tx_hash)
        }
    }
}