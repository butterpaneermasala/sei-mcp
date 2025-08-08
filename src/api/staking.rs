// src/api/staking.rs

use crate::{
    blockchain::{
        client::SeiClient,
        models::{ClaimRewardsRequest, StakeRequest, UnstakeRequest, ValidatorInfo},
    },
    config::AppConfig,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

// --- Response Models ---

#[derive(Debug, Serialize)]
pub struct StakingResponse {
    pub tx_hash: String,
}

#[derive(Debug, Serialize)]
pub struct ValidatorsResponse {
    pub validators: Vec<ValidatorInfo>,
}

#[derive(Debug, Serialize)]
pub struct AprResponse {
    pub apr: String,
}

// --- Handlers ---

pub async fn stake_handler(
    Path(chain_id): Path<String>,
    State(config): State<AppConfig>,
    Json(request): Json<StakeRequest>,
) -> Result<Json<StakingResponse>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls);
    match client.stake_tokens(&chain_id, &request).await {
        Ok(response) => Ok(Json(StakingResponse {
            tx_hash: response.tx_hash,
        })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to stake tokens: {}", e),
        )),
    }
}

pub async fn unstake_handler(
    Path(chain_id): Path<String>,
    State(config): State<AppConfig>,
    Json(request): Json<UnstakeRequest>,
) -> Result<Json<StakingResponse>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls);
    match client.unstake_tokens(&chain_id, &request).await {
        Ok(response) => Ok(Json(StakingResponse {
            tx_hash: response.tx_hash,
        })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to unstake tokens: {}", e),
        )),
    }
}

pub async fn claim_rewards_handler(
    Path(chain_id): Path<String>,
    State(config): State<AppConfig>,
    Json(request): Json<ClaimRewardsRequest>,
) -> Result<Json<StakingResponse>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls);
    match client.claim_rewards(&chain_id, &request).await {
        Ok(response) => Ok(Json(StakingResponse {
            tx_hash: response.tx_hash,
        })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to claim rewards: {}", e),
        )),
    }
}

pub async fn get_validators_handler(
    Path(chain_id): Path<String>,
    State(config): State<AppConfig>,
) -> Result<Json<ValidatorsResponse>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls);
    match client.get_all_validators(&chain_id).await {
        Ok(validators) => Ok(Json(ValidatorsResponse { validators })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get validators: {}", e),
        )),
    }
}

pub async fn get_apr_handler(
    Path(chain_id): Path<String>,
    State(config): State<AppConfig>,
) -> Result<Json<AprResponse>, (axum::http::StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls);
    match client.get_staking_apr(&chain_id).await {
        Ok(apr) => Ok(Json(AprResponse { apr })),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get staking APR: {}", e),
        )),
    }
}
