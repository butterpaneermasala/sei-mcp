// src/blockchain/services/transactions.rs

use anyhow::{anyhow, Result};
use ethers_core::abi::{Function, Param, ParamType, StateMutability, Token};
use ethers_core::types::{Address, Bytes, TransactionRequest, U256, U64};
use ethers_signers::{LocalWallet, Signer};
use reqwest::Client;
use serde_json::json;
use std::str::FromStr;
use tracing::info;

use crate::blockchain::models::{
    ApproveRequest, NftTransferRequest, SeiTransferRequest, TokenInfoResponse,
    TokenTransferRequest, TransactionResponse,
};

/// Transfers native SEI tokens.
pub async fn transfer_sei(
    client: &Client,
    rpc_url: &str,
    request: &SeiTransferRequest,
    private_key: &str,
) -> Result<TransactionResponse> {
    info!("Initiating SEI transfer");
    let wallet = LocalWallet::from_str(private_key)?;
    let to_address = Address::from_str(&request.to_address)?;
    let value = U256::from_dec_str(&request.amount)?;

    // Get nonce for the transaction
    let nonce_payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [wallet.address(), "latest"],
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
        .ok_or_else(|| anyhow!("Failed to get nonce"))?;
    let nonce = U256::from_str(nonce_hex).map_err(|_| anyhow!("Failed to parse nonce"))?;

    // Get chain id
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
        .ok_or_else(|| anyhow!("Failed to get chain id"))?;
    let chain_id = U64::from_str(chain_id_hex).map_err(|_| anyhow!("Failed to parse chain id"))?;

    let gas_limit = if let Some(limit) = &request.gas_limit {
        U256::from_dec_str(limit).unwrap_or(U256::from(30000))
    } else {
        U256::from(30000)
    };

    let gas_price = if let Some(price) = &request.gas_price {
        U256::from_dec_str(price).unwrap_or(U256::from(1500000000))
    } else {
        U256::from(1500000000)
    };

    let mut tx = TransactionRequest::new()
        .to(to_address)
        .value(value)
        .from(wallet.address())
        .nonce(nonce)
        .chain_id(chain_id.as_u64())
        .gas(gas_limit)
        .gas_price(gas_price);

    info!("Sending transaction with parameters:");
    info!("From: {:?}", wallet.address());
    info!("To: {:?}", to_address);
    info!("Value: {:?}", value);
    info!("Nonce: {:?}", nonce);
    info!("Chain ID: {:?}", chain_id);
    info!("Gas Limit: {:?}", gas_limit);
    info!("Gas Price: {:?}", gas_price);
    send_transaction(client, rpc_url, wallet, tx).await
}

/// Transfers ERC20 tokens.
pub async fn transfer_erc20(
    client: &Client,
    rpc_url: &str,
    request: &TokenTransferRequest,
    private_key: &str,
) -> Result<TransactionResponse> {
    info!("Initiating ERC20 transfer");
    let wallet = LocalWallet::from_str(private_key)?;
    let to_address = Address::from_str(&request.to_address)?;
    let contract_address = Address::from_str(&request.contract_address)?;
    let amount = U256::from_dec_str(&request.amount)?;

    let data = erc20_transfer_data(to_address, amount)?;

    let tx = TransactionRequest::new()
        .to(contract_address)
        .data(data)
        .from(wallet.address());

    send_transaction(client, rpc_url, wallet, tx).await
}

/// Transfers an NFT (ERC721 or ERC1155).
pub async fn transfer_nft(
    client: &Client,
    rpc_url: &str,
    request: &NftTransferRequest,
    private_key: &str,
) -> Result<TransactionResponse> {
    info!("Initiating NFT transfer");
    let wallet = LocalWallet::from_str(private_key)?;
    let from_address = wallet.address();
    let to_address = Address::from_str(&request.to_address)?;
    let contract_address = Address::from_str(&request.contract_address)?;
    let token_id = U256::from_dec_str(&request.token_id)?;

    // This uses the `safeTransferFrom` function for broader compatibility (ERC721 & ERC1155)
    let data = nft_transfer_data(from_address, to_address, token_id)?;

    let tx = TransactionRequest::new()
        .to(contract_address)
        .data(data)
        .from(from_address);

    send_transaction(client, rpc_url, wallet, tx).await
}

/// Approves spending of an ERC20 token.
pub async fn approve_token(
    client: &Client,
    rpc_url: &str,
    request: &ApproveRequest,
    private_key: &str,
) -> Result<TransactionResponse> {
    info!("Initiating token approval");
    let wallet = LocalWallet::from_str(private_key)?;
    let spender_address = Address::from_str(&request.spender_address)?;
    let contract_address = Address::from_str(&request.contract_address)?;
    let amount = U256::from_dec_str(&request.amount)?;

    let data = approve_data(spender_address, amount)?;

    let tx = TransactionRequest::new()
        .to(contract_address)
        .data(data)
        .from(wallet.address());

    send_transaction(client, rpc_url, wallet, tx).await
}

/// Retrieves information about a token.
pub async fn get_token_info(
    client: &Client,
    rpc_url: &str,
    contract_address: &str,
) -> Result<TokenInfoResponse> {
    info!("Fetching token info for {}", contract_address);
    let address = Address::from_str(contract_address)?;

    let name: String = call_contract_function(client, rpc_url, address, "name", &[]).await?;
    let symbol: String = call_contract_function(client, rpc_url, address, "symbol", &[]).await?;
    let decimals: U256 = call_contract_function(client, rpc_url, address, "decimals", &[]).await?;

    Ok(TokenInfoResponse {
        name,
        symbol,
        decimals: decimals.as_u64(),
        contract_address: contract_address.to_string(),
    })
}

// --- Helper Functions ---

/// Signs and sends a transaction.
async fn send_transaction(
    client: &Client,
    rpc_url: &str,
    wallet: LocalWallet,
    tx: TransactionRequest,
) -> Result<TransactionResponse> {
    let signature = wallet.sign_transaction(&tx.clone().into()).await?;
    let raw_tx = tx.rlp_signed(&signature);

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

    if let Some(error) = response.get("error") {
        return Err(anyhow!("RPC Error: {}", error));
    }

    let tx_hash = response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to extract transaction hash from response"))?;

    Ok(TransactionResponse {
        tx_hash: tx_hash.to_string(),
    })
}

/// Calls a read-only function on a smart contract.
async fn call_contract_function<T: ethers_core::abi::Detokenize>(
    client: &Client,
    rpc_url: &str,
    contract: Address,
    function_name: &str,
    params: &[Token],
) -> Result<T> {
    // NOTE: You must provide the correct parameter types for the function being called.
    // For ERC20 name/symbol/decimals, there are no inputs, so this is fine.
    let function = Function {
        name: function_name.to_string(),
        inputs: vec![], // No inputs for name/symbol/decimals
        outputs: vec![Param {
            name: "output".to_string(),
            kind: ParamType::String, // Placeholder, will be decoded to T
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::View,
    };

    let data = function.encode_input(params)?;

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{
            "to": contract,
            "data": format!("0x{}", hex::encode(data)),
        }, "latest"],
        "id": 1
    });

    let response: serde_json::Value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    let result_hex = response["result"]
        .as_str()
        .ok_or_else(|| anyhow!("eth_call failed"))?;
    let result_bytes = hex::decode(result_hex.strip_prefix("0x").unwrap_or(result_hex))?;

    let tokens = function.decode_output(&result_bytes)?;
    T::from_tokens(tokens).map_err(|e| anyhow!("Failed to detokenize result: {:?}", e))
}

fn erc20_transfer_data(to: Address, amount: U256) -> Result<Bytes> {
    let function = Function {
        name: "transfer".to_string(),
        inputs: vec![
            Param {
                name: "_to".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_value".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![Param {
            name: "success".to_string(),
            kind: ParamType::Bool,
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    };
    let data = function.encode_input(&[Token::Address(to), Token::Uint(amount)])?;
    Ok(data.into())
}

fn nft_transfer_data(from: Address, to: Address, token_id: U256) -> Result<Bytes> {
    let function = Function {
        name: "safeTransferFrom".to_string(),
        inputs: vec![
            Param {
                name: "_from".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_to".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_tokenId".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    };
    let data = function.encode_input(&[
        Token::Address(from),
        Token::Address(to),
        Token::Uint(token_id),
    ])?;
    Ok(data.into())
}

fn approve_data(spender: Address, amount: U256) -> Result<Bytes> {
    let function = Function {
        name: "approve".to_string(),
        inputs: vec![
            Param {
                name: "_spender".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_value".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![Param {
            name: "success".to_string(),
            kind: ParamType::Bool,
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    };
    let data = function.encode_input(&[Token::Address(spender), Token::Uint(amount)])?;
    Ok(data.into())
}
