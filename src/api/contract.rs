// src/api/contract.rs

use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct ContractPath {
    pub address: String,
}

pub async fn get_contract_handler(
    State(state): State<AppState>,
    Path(params): Path<ContractPath>,
) -> impl IntoResponse {
    match state
        .sei_client
        .get_contract("sei-evm-testnet", &params.address)
        .await
    {
        Ok(contract) => (StatusCode::OK, Json(contract)).into_response(),
        Err(e) => {
            error!("Failed to get contract: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

pub async fn get_contract_code_handler(
    State(state): State<AppState>,
    Path(params): Path<ContractPath>,
) -> impl IntoResponse {
    match state
        .sei_client
        .get_contract_code("sei-evm-testnet", &params.address)
        .await
    {
        Ok(code) => (StatusCode::OK, Json(code)).into_response(),
        Err(e) => {
            error!("Failed to get contract code: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

pub async fn get_contract_transactions_handler(
    State(state): State<AppState>,
    Path(params): Path<ContractPath>,
) -> impl IntoResponse {
    match state
        .sei_client
        .get_contract_transactions("sei-evm-testnet", &params.address)
        .await
    {
        Ok(txs) => (StatusCode::OK, Json(txs)).into_response(),
        Err(e) => {
            error!("Failed to get contract transactions: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
