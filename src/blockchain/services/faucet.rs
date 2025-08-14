// src/blockchain/services/faucet.rs

use crate::config::Config;
use anyhow::{anyhow, Result};
use ethers_core::types::{Address, TransactionRequest, U256, U64};
use ethers_signers::{LocalWallet, Signer};
use reqwest::Client as ReqwestClient;
use serde_json::json;
use std::str::FromStr;
use tracing::info;

/// Sends faucet tokens to a specified EVM address using the ethers-rs library.
/// This function constructs, signs, and sends a standard EVM transaction.
pub async fn send_faucet_tokens(
    config: &Config,
    recipient_address: &str,
) -> Result<String> {
    info!(
        "Initiating EVM faucet transfer to address: {}",
        recipient_address
    );

    // 1. Initialize wallet from the faucet's private key stored in the config.
    let wallet = LocalWallet::from_str(&config.faucet_private_key)
        .map_err(|e| anyhow!("Failed to create wallet from faucet private key: {}", e))?;
    let from_address = wallet.address();

    // 2. Parse the recipient address and the faucet amount.
    let to_address = Address::from_str(recipient_address)
        .map_err(|e| anyhow!("Invalid recipient EVM address format: {}", e))?;
    let value = U256::from(config.faucet_amount_usei);

    // 3. Create an HTTP client and get the RPC URL for the testnet.
    let client = ReqwestClient::new();
    let rpc_url = config
        .chain_rpc_urls
        .get("sei-testnet") // Assuming the faucet is always for the testnet
        .ok_or_else(|| anyhow!("'sei-testnet' RPC URL not found in configuration"))?;

    // 4. Get the nonce for the transaction by calling `eth_getTransactionCount`.
    let nonce_payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [from_address, "latest"],
        "id": 1
    });
    let nonce_response: serde_json::Value = client
        .post(rpc_url)
        .json(&nonce_payload)
        .send()
        .await?
        .json()
        .await?;
    let nonce_hex = nonce_response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get nonce from RPC response: {:?}", nonce_response))?;
    let nonce = U256::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)
        .map_err(|_| anyhow!("Failed to parse nonce hex: {}", nonce_hex))?;

    // 5. Get the chain ID by calling `eth_chainId`.
    let chain_id_payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_chainId",
        "params": [],
        "id": 1
    });
    let chain_id_response: serde_json::Value = client
        .post(rpc_url)
        .json(&chain_id_payload)
        .send()
        .await?
        .json()
        .await?;
    let chain_id_hex = chain_id_response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get chain_id from RPC response: {:?}", chain_id_response))?;
    let chain_id = U64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)
        .map_err(|_| anyhow!("Failed to parse chain_id hex: {}", chain_id_hex))?;

    // 6. Construct the full EVM transaction request.
    let tx = TransactionRequest::new()
        .to(to_address)
        .value(value)
        .from(from_address)
        .nonce(nonce)
        .chain_id(chain_id.as_u64())
        .gas(U256::from(config.faucet_gas_limit))
        .gas_price(U256::from(config.faucet_fee_amount)); // Using faucet_fee_amount as gas_price

    info!("Sending faucet transaction with parameters: {:?}", tx);

    // 7. Sign the transaction with the faucet's wallet and serialize it.
    let signature = wallet.sign_transaction(&tx.clone().into()).await?;
    let raw_tx = tx.rlp_signed(&signature);

    // 8. Send the raw transaction via `eth_sendRawTransaction`.
    let params = json!([raw_tx]);
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": params,
        "id": 1,
    });

    let response: serde_json::Value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    info!("Received faucet send response: {:?}", response);

    if let Some(error) = response.get("error") {
        return Err(anyhow!("RPC Error sending faucet transaction: {}", error));
    }

    // 9. Extract and return the transaction hash on success.
    let tx_hash = response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to extract transaction hash from faucet response"))?;

    Ok(tx_hash.to_string())
}
