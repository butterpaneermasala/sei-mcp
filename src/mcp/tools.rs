use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use crate::mcp::protocol::Tool;
use crate::mcp::enhanced_tools::list_enhanced_tools;
use crate::blockchain::client::SeiClient;

pub async fn call_get_balance(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let address = args_map
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing address parameter"))?;

    let balance = client.get_balance(chain_id, address).await?;
    Ok(json!({
        "chain_id": chain_id,
        "address": address,
        "balance": balance.amount,
        "denom": balance.denom
    }))
}

pub async fn call_create_wallet(client: &SeiClient, _args: Option<Value>) -> Result<Value> {
    let wallet = client.create_wallet().await?;
    Ok(json!({
        "address": wallet.address,
        "private_key": wallet.private_key,
        "mnemonic": wallet.mnemonic
    }))
}

pub async fn call_import_wallet(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let mnemonic_or_key = args_map
        .get("mnemonic_or_private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing mnemonic_or_private_key parameter"))?;

    let wallet = client.import_wallet(mnemonic_or_key).await?;
    Ok(json!({
        "address": wallet.address,
        "private_key": wallet.private_key,
        "mnemonic": wallet.mnemonic
    }))
}

pub async fn call_get_transaction_history(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let address = args_map
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing address parameter"))?;

    let limit = args_map
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20);

    let history = client.get_transaction_history(chain_id, address, limit).await?;
    Ok(json!({
        "address": address,
        "chain_id": chain_id,
        "transactions": history.transactions
    }))
}

pub async fn call_estimate_fees(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let from = args_map
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing from parameter"))?;

    let to = args_map
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing to parameter"))?;

    let amount = args_map
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing amount parameter"))?;

    let request = crate::blockchain::models::EstimateFeesRequest {
        from: from.to_string(),
        to: to.to_string(),
        amount: amount.to_string(),
    };

    let fees = client.estimate_fees(chain_id, &request).await?;
    Ok(json!({
        "estimated_gas": fees.estimated_gas,
        "gas_price": fees.gas_price,
        "total_fee": fees.total_fee,
        "denom": fees.denom
    }))
}

pub async fn call_transfer_sei(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let to_address = args_map
        .get("to_address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing to_address parameter"))?;

    let amount = args_map
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing amount parameter"))?;

    let private_key = args_map
        .get("private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing private_key parameter"))?;

    let gas_limit = args_map
        .get("gas_limit")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let gas_price = args_map
        .get("gas_price")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let request = crate::blockchain::models::SeiTransferRequest {
        to_address: to_address.to_string(),
        amount: amount.to_string(),
        private_key: private_key.to_string(),
        gas_limit,
        gas_price,
    };

    let response = client.transfer_sei(chain_id, &request).await?;
    Ok(json!({
        "chain_id": chain_id,
        "tx_hash": response.tx_hash
    }))
}

pub fn list_tools() -> Vec<Tool> {
    let mut tools = vec![
        Tool {
            name: "get_balance".into(),
            description: "Get the balance of an address on a specific blockchain".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID (e.g., 'sei')"
                    },
                    "address": {
                        "type": "string",
                        "description": "The wallet address to check balance for"
                    }
                },
                "required": ["chain_id", "address"]
            }),
        },
        Tool {
            name: "create_wallet".into(),
            description: "Create a new wallet with mnemonic phrase".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        Tool {
            name: "import_wallet".into(),
            description: "Import a wallet from mnemonic phrase or private key".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "mnemonic_or_private_key": {
                        "type": "string",
                        "description": "The mnemonic phrase or private key to import"
                    }
                },
                "required": ["mnemonic_or_private_key"]
            }),
        },
        Tool {
            name: "get_transaction_history".into(),
            description: "Get transaction history for an address (Sei chain only)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID (currently only 'sei' is supported)"
                    },
                    "address": {
                        "type": "string",
                        "description": "The wallet address to get history for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of transactions to return (default: 20, max: 100)",
                        "minimum": 1,
                        "maximum": 100
                    }
                },
                "required": ["chain_id", "address"]
            }),
        },
        Tool {
            name: "estimate_fees".into(),
            description: "Estimate transaction fees for a transfer".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID"
                    },
                    "from": {
                        "type": "string",
                        "description": "The sender address"
                    },
                    "to": {
                        "type": "string",
                        "description": "The recipient address"
                    },
                    "amount": {
                        "type": "string",
                        "description": "The amount to send"
                    }
                },
                "required": ["chain_id", "from", "to", "amount"]
            }),
        },
        Tool {
            name: "transfer_sei".into(),
            description: "Transfer SEI tokens to another address".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID"
                    },
                    "to_address": {
                        "type": "string",
                        "description": "The recipient address"
                    },
                    "amount": {
                        "type": "string",
                        "description": "The amount of SEI tokens to transfer"
                    },
                    "private_key": {
                        "type": "string",
                        "description": "The private key of the sender wallet"
                    },
                    "gas_limit": {
                        "type": "string",
                        "description": "Optional gas limit (default: 100000)"
                    },
                    "gas_price": {
                        "type": "string",
                        "description": "Optional gas price (default: 20000000000)"
                    }
                },
                "required": ["chain_id", "to_address", "amount", "private_key"]
            }),
        }
    ];
    
    // Add enhanced tools
    tools.extend(list_enhanced_tools());
    
    tools
}
