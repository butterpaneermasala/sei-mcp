// src/blockchain/services/contract.rs

use crate::blockchain::models::{Contract, ContractCode, ContractTransactionsResponse};
use anyhow::{anyhow, Result};
use reqwest::Client;

const SEISTREAM_API_BASE: &str = "https://api.seistream.app/contracts/evm";

pub async fn get_contract(client: &Client, address: &str) -> Result<Contract> {
    let url = format!("{}/{}", SEISTREAM_API_BASE, address);
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        Ok(res.json::<Contract>().await?)
    } else {
        Err(anyhow!("Failed to get contract info: {}", res.status()))
    }
}

pub async fn get_contract_code(client: &Client, address: &str) -> Result<ContractCode> {
    let url = format!("{}/{}/code", SEISTREAM_API_BASE, address);
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        Ok(res.json::<ContractCode>().await?)
    } else {
        Err(anyhow!("Failed to get contract code: {}", res.status()))
    }
}

pub async fn get_contract_transactions(
    client: &Client,
    address: &str,
) -> Result<ContractTransactionsResponse> {
    let url = format!("{}/{}/transactions", SEISTREAM_API_BASE, address);
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        Ok(res.json::<ContractTransactionsResponse>().await?)
    } else {
        Err(anyhow!(
            "Failed to get contract transactions: {}",
            res.status()
        ))
    }
}
