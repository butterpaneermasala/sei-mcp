// src/blockchain/services/staking.rs

use anyhow::{anyhow, Result};
use cosmrs::{crypto::secp256k1, rpc::Client as RpcClient};
use reqwest::Client as HttpClient;
use std::str::FromStr;
use tracing::info;

use crate::blockchain::models::{
    AllValidatorsResponse, ClaimRewardsRequest, StakeRequest, StakingAprResponse,
    TransactionResponse, UnstakeRequest, ValidatorInfo,
};

/// Helper to get network-specific parameters
fn get_network_params(chain_id: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match chain_id {
        "sei" | "pacific-1" => Ok((
            "pacific-1",
            "https://rpc.sei-apis.com",
            "https://rest.sei-apis.com",
        )),
        "sei-testnet" | "atlantic-2" => Ok((
            "atlantic-2",
            "https://rpc-testnet.sei-apis.com",
            "https://rest-testnet.sei-apis.com",
        )),
        _ => Err(anyhow!("Unsupported chain_id for staking: {}", chain_id)),
    }
}

/// Helper function to create a signer from a hex private key
fn create_signer_from_hex_private_key(private_key_hex: &str) -> Result<secp256k1::SigningKey> {
    let pk_bytes = hex::decode(private_key_hex.trim_start_matches("0x"))?;
    secp256k1::SigningKey::from_slice(&pk_bytes)
        .map_err(|e| anyhow!("Failed to create signing key: {}", e))
}

/// Validate staking amount
fn validate_staking_amount(amount: &str) -> Result<u128> {
    let amount_u128 =
        u128::from_str(amount).map_err(|_| anyhow!("Invalid staking amount: {}", amount))?;

    if amount_u128 == 0 {
        return Err(anyhow!("Staking amount must be greater than 0"));
    }

    // Minimum staking amount (1 SEI = 1_000_000 usei)
    if amount_u128 < 1_000_000 {
        return Err(anyhow!("Minimum staking amount is 1 SEI (1,000,000 usei)"));
    }

    Ok(amount_u128)
}

/// Validate validator address
fn validate_validator_address(address: &str) -> Result<()> {
    if address.is_empty() {
        return Err(anyhow!("Validator address cannot be empty"));
    }

    // Basic format validation for SEI validator addresses
    if !address.starts_with("seivaloper") {
        return Err(anyhow!(
            "Invalid validator address format. Expected 'seivaloper' prefix"
        ));
    }

    if address.len() < 45 || address.len() > 50 {
        return Err(anyhow!("Invalid validator address length"));
    }

    Ok(())
}

/// Generic function to build, sign, and broadcast a Cosmos transaction
/// Note: This is a simplified implementation that returns a placeholder response
/// In a production environment, you would implement the full transaction signing and broadcasting
async fn sign_and_broadcast_tx(
    _rpc_url: &str,
    _msg: cosmrs::Any,
    _signer: &secp256k1::SigningKey,
    _fee_amount: u64,
    _chain_id_str: &str,
) -> Result<TransactionResponse> {
    // TODO: Implement full transaction signing and broadcasting
    // This would involve:
    // 1. Getting account details from the blockchain
    // 2. Creating and signing the transaction
    // 3. Broadcasting the transaction
    // 4. Handling the response

    // For now, return a placeholder response
    Ok(TransactionResponse {
        tx_hash: format!("placeholder_tx_{}", chrono::Utc::now().timestamp()),
    })
}

/// Stakes (delegates) tokens to a validator.
pub async fn stake_tokens(
    _http_client: &HttpClient,
    request: &StakeRequest,
    chain_id: &str,
) -> Result<TransactionResponse> {
    // Validate inputs
    validate_validator_address(&request.validator_address)?;
    let _amount_u128 = validate_staking_amount(&request.amount)?;

    let (network_chain_id, _, _) = get_network_params(chain_id)?;
    info!(
        "Staking {} usei to validator {} on chain {}",
        request.amount, request.validator_address, network_chain_id
    );

    // Create signer from private key for validation
    let signer = create_signer_from_hex_private_key(&request.private_key)?;
    let _delegator_address = signer
        .public_key()
        .account_id("sei")
        .map_err(|e| anyhow!("Failed to create delegator address: {}", e))?;
    let _validator_address = cosmrs::AccountId::from_str(&request.validator_address)
        .map_err(|e| anyhow!("Failed to parse validator address: {}", e))?;

    // TODO: Implement actual transaction signing and broadcasting
    // For now, return a placeholder response with validation
    Ok(TransactionResponse {
        tx_hash: format!(
            "stake_tx_{}_{}_{}",
            request.validator_address,
            request.amount,
            chrono::Utc::now().timestamp()
        ),
    })
}

/// Unstakes (unbonds) tokens from a validator.
pub async fn unstake_tokens(
    _http_client: &HttpClient,
    request: &UnstakeRequest,
    chain_id: &str,
) -> Result<TransactionResponse> {
    // Validate inputs
    validate_validator_address(&request.validator_address)?;
    let _amount_u128 = validate_staking_amount(&request.amount)?;

    let (network_chain_id, _, _) = get_network_params(chain_id)?;
    info!(
        "Unstaking {} usei from validator {} on chain {}",
        request.amount, request.validator_address, network_chain_id
    );

    // Create signer from private key for validation
    let signer = create_signer_from_hex_private_key(&request.private_key)?;
    let _delegator_address = signer
        .public_key()
        .account_id("sei")
        .map_err(|e| anyhow!("Failed to create delegator address: {}", e))?;
    let _validator_address = cosmrs::AccountId::from_str(&request.validator_address)
        .map_err(|e| anyhow!("Failed to parse validator address: {}", e))?;

    // TODO: Implement actual transaction signing and broadcasting
    // For now, return a placeholder response with validation
    Ok(TransactionResponse {
        tx_hash: format!(
            "unstake_tx_{}_{}_{}",
            request.validator_address,
            request.amount,
            chrono::Utc::now().timestamp()
        ),
    })
}

/// Claims staking rewards from a validator.
pub async fn claim_rewards(
    _http_client: &HttpClient,
    request: &ClaimRewardsRequest,
    chain_id: &str,
) -> Result<TransactionResponse> {
    // Validate inputs
    validate_validator_address(&request.validator_address)?;

    let (network_chain_id, _, _) = get_network_params(chain_id)?;
    info!(
        "Claiming rewards from validator {} on chain {}",
        request.validator_address, network_chain_id
    );

    // Create signer from private key for validation
    let signer = create_signer_from_hex_private_key(&request.private_key)?;
    let _delegator_address = signer
        .public_key()
        .account_id("sei")
        .map_err(|e| anyhow!("Failed to create delegator address: {}", e))?;
    let _validator_address = cosmrs::AccountId::from_str(&request.validator_address)
        .map_err(|e| anyhow!("Failed to parse validator address: {}", e))?;

    // TODO: Implement actual transaction signing and broadcasting
    // For now, return a placeholder response with validation
    Ok(TransactionResponse {
        tx_hash: format!(
            "claim_rewards_tx_{}_{}",
            request.validator_address,
            chrono::Utc::now().timestamp()
        ),
    })
}

/// Fetches information about all validators from the REST endpoint.
pub async fn get_all_validators(
    http_client: &HttpClient,
    chain_id: &str,
) -> Result<Vec<ValidatorInfo>> {
    let (_, _, rest_url) = get_network_params(chain_id)?;
    info!("Fetching all validators from REST endpoint: {}", rest_url);
    let url = format!("{}/cosmos/staking/v1beta1/validators", rest_url);
    let res = http_client
        .get(&url)
        .send()
        .await?
        .json::<AllValidatorsResponse>()
        .await?;
    Ok(res.validators)
}

/// Fetches the current staking APR from a public endpoint.
pub async fn get_staking_apr(http_client: &HttpClient, chain_id: &str) -> Result<String> {
    let api_url = match chain_id {
        "sei" | "pacific-1" => "https://api.seistream.app/staking/apr",
        // Note: Seistream may not have a testnet APR endpoint. This is a placeholder.
        "sei-testnet" | "atlantic-2" => "https://api-testnet.seistream.app/staking/apr",
        _ => return Err(anyhow!("No APR endpoint for chain_id: {}", chain_id)),
    };
    info!("Fetching staking APR from: {}", api_url);
    let res = http_client
        .get(api_url)
        .send()
        .await?
        .json::<StakingAprResponse>()
        .await?;
    Ok(res.staking_apr)
}
