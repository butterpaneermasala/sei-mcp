use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use crate::mcp::protocol::Tool;
use crate::mcp::wallet_storage::{
    initialize_wallet_storage, add_wallet_to_storage, get_wallet_from_storage,
    get_decrypted_private_key_from_storage, list_wallets_from_storage, remove_wallet_from_storage
};
use crate::blockchain::client::SeiClient;
use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;
use rand::Rng;
use rand::distributions::Alphanumeric;
use chrono::{DateTime, Utc};

// Global pending transactions storage (in-memory only, as these are temporary)
lazy_static! {
    static ref PENDING_TRANSACTIONS: Mutex<HashMap<String, PendingTransaction>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    transaction_id: String,
    wallet_name: String,
    to_address: String,
    amount: String,
    chain_id: String,
    gas_limit: Option<String>,
    gas_price: Option<String>,
    confirmation_code: String,
    created_at: DateTime<Utc>,
}

fn generate_confirmation_code() -> String {
    // Generate a more secure confirmation code with mixed case and numbers
    let mut rng = rand::thread_rng();
    let mut code = String::new();
    
    // Add 3 random uppercase letters
    for _ in 0..3 {
        code.push(rng.gen_range('A'..='Z'));
    }
    
    // Add 3 random numbers
    for _ in 0..3 {
        code.push(rng.gen_range('0'..='9'));
    }
    
    // Shuffle the characters by collecting into a vector and shuffling
    let mut chars: Vec<char> = code.chars().collect();
    for i in (1..chars.len()).rev() {
        let j = rng.gen_range(0..=i);
        chars.swap(i, j);
    }
    chars.into_iter().collect()
}

fn generate_transaction_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

pub async fn call_register_wallet(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let wallet_name = args_map
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;

    let private_key = args_map
        .get("private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing private_key parameter"))?;

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    // Initialize wallet storage with master password
    initialize_wallet_storage(master_password)?;

    // Validate private key by creating a wallet
    let wallet = client.import_wallet(private_key).await
        .map_err(|_| anyhow!("Invalid private key"))?;

    // Add wallet to persistent storage
    add_wallet_to_storage(
        wallet_name.to_string(),
        private_key.to_string(),
        wallet.address.clone(),
        master_password
    )?;

    Ok(json!({
        "wallet_name": wallet_name,
        "address": wallet.address,
        "status": "registered",
        "message": "Wallet registered successfully with encrypted private key and stored locally"
    }))
}

pub async fn call_list_wallets(_client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    // Initialize wallet storage
    initialize_wallet_storage(master_password)?;

    let wallets = list_wallets_from_storage()?;
    let wallets_json: Vec<Value> = wallets.iter().map(|wallet| {
        json!({
            "wallet_name": wallet.wallet_name,
            "address": wallet.public_address,
            "created_at": wallet.created_at.to_rfc3339(),
            "updated_at": wallet.updated_at.to_rfc3339()
        })
    }).collect();

    Ok(json!({
        "wallets": wallets_json,
        "count": wallets_json.len()
    }))
}

pub async fn call_get_wallet_balance(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let wallet_name = args_map
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    // Initialize wallet storage
    initialize_wallet_storage(master_password)?;

    // Get wallet from storage
    let wallet = get_wallet_from_storage(wallet_name, master_password)?;

    let balance = client.get_balance(chain_id, &wallet.public_address).await?;
    
    Ok(json!({
        "wallet_name": wallet_name,
        "chain_id": chain_id,
        "address": wallet.public_address,
        "balance": balance.amount,
        "denom": balance.denom
    }))
}

pub async fn call_transfer_from_wallet(_client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let wallet_name = args_map
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;

    let to_address = args_map
        .get("to_address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing to_address parameter"))?;

    let amount = args_map
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing amount parameter"))?;

    let chain_id = args_map
        .get("chain_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    let gas_limit = args_map
        .get("gas_limit")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let gas_price = args_map
        .get("gas_price")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Initialize wallet storage
    initialize_wallet_storage(master_password)?;

    // Get wallet from storage
    let wallet = get_wallet_from_storage(wallet_name, master_password)?;

    // Generate transaction ID and confirmation code
    let transaction_id = generate_transaction_id();
    let confirmation_code = generate_confirmation_code();

    // Store pending transaction
    let pending_tx = PendingTransaction {
        transaction_id: transaction_id.clone(),
        wallet_name: wallet_name.to_string(),
        to_address: to_address.to_string(),
        amount: amount.to_string(),
        chain_id: chain_id.to_string(),
        gas_limit,
        gas_price,
        confirmation_code: confirmation_code.clone(),
        created_at: Utc::now(),
    };

    let mut pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
    pending_storage.insert(transaction_id.clone(), pending_tx);

    Ok(json!({
        "transaction_id": transaction_id,
        "confirmation_code": confirmation_code,
        "message": format!("To confirm this transfer, please provide the confirmation code: {}", confirmation_code),
        "details": {
            "from_wallet": wallet_name,
            "from_address": wallet.public_address,
            "to_address": to_address,
            "amount": amount,
            "chain_id": chain_id
        }
    }))
}

pub async fn call_confirm_transaction(client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let transaction_id = args_map
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing transaction_id parameter"))?;

    let confirmation_code = args_map
        .get("confirmation_code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing confirmation_code parameter"))?;

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    // Get pending transaction and verify confirmation code
    let pending_tx = {
        let pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
        let pending_tx = pending_storage.get(transaction_id)
            .ok_or_else(|| anyhow!("Transaction '{}' not found or expired", transaction_id))?;

        // Verify confirmation code
        if pending_tx.confirmation_code != confirmation_code {
            return Err(anyhow!("Invalid confirmation code"));
        }

        // Check if transaction is expired (5 minutes)
        let now = Utc::now();
        if (now - pending_tx.created_at).num_minutes() > 5 {
            return Err(anyhow!("Transaction expired"));
        }

        pending_tx.clone()
    };

    // Initialize wallet storage
    initialize_wallet_storage(master_password)?;

    // Get decrypted private key from storage
    let private_key = get_decrypted_private_key_from_storage(&pending_tx.wallet_name, master_password)?;

    // Create transfer request
    let request = crate::blockchain::models::SeiTransferRequest {
        to_address: pending_tx.to_address.clone(),
        amount: pending_tx.amount.clone(),
        private_key: private_key,
        gas_limit: pending_tx.gas_limit.clone(),
        gas_price: pending_tx.gas_price.clone(),
    };

    // Execute transfer
    let response = client.transfer_sei(&pending_tx.chain_id, &request).await?;

    // Remove pending transaction
    {
        let mut pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
        pending_storage.remove(transaction_id);
    }

    Ok(json!({
        "transaction_id": transaction_id,
        "status": "confirmed",
        "tx_hash": response.tx_hash,
        "chain_id": pending_tx.chain_id,
        "details": {
            "from_wallet": pending_tx.wallet_name,
            "to_address": pending_tx.to_address,
            "amount": pending_tx.amount
        }
    }))
}

pub async fn call_remove_wallet(_client: &SeiClient, args: Option<Value>) -> Result<Value> {
    let args_map = args
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let wallet_name = args_map
        .get("wallet_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;

    let master_password = args_map
        .get("master_password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

    // Initialize wallet storage
    initialize_wallet_storage(master_password)?;

    let removed = remove_wallet_from_storage(wallet_name)?;
    if removed {
        Ok(json!({
            "wallet_name": wallet_name,
            "status": "removed",
            "message": "Wallet removed from local storage"
        }))
    } else {
        Err(anyhow!("Wallet '{}' not found", wallet_name))
    }
}

pub fn list_enhanced_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "register_wallet".into(),
            description: "Register a wallet for secure storage and usage".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "wallet_name": {
                        "type": "string",
                        "description": "A unique name for this wallet"
                    },
                    "private_key": {
                        "type": "string",
                        "description": "The private key of the wallet to register"
                    },
                    "master_password": {
                        "type": "string",
                        "description": "The master password to encrypt the private key"
                    }
                },
                "required": ["wallet_name", "private_key", "master_password"]
            }),
        },
        Tool {
            name: "list_wallets".into(),
            description: "List all registered wallets".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "master_password": {
                        "type": "string",
                        "description": "The master password to decrypt wallet private keys"
                    }
                },
                "required": ["master_password"]
            }),
        },
        Tool {
            name: "get_wallet_balance".into(),
            description: "Get the balance of a registered wallet".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "wallet_name": {
                        "type": "string",
                        "description": "The name of the registered wallet"
                    },
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID (e.g., 'sei')"
                    },
                    "master_password": {
                        "type": "string",
                        "description": "The master password to decrypt wallet private keys"
                    }
                },
                "required": ["wallet_name", "chain_id", "master_password"]
            }),
        },
        Tool {
            name: "transfer_from_wallet".into(),
            description: "Initiate a transfer from a registered wallet (requires confirmation)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "wallet_name": {
                        "type": "string",
                        "description": "The name of the registered wallet to send from"
                    },
                    "to_address": {
                        "type": "string",
                        "description": "The recipient address"
                    },
                    "amount": {
                        "type": "string",
                        "description": "The amount of tokens to transfer"
                    },
                    "chain_id": {
                        "type": "string",
                        "description": "The blockchain chain ID"
                    },
                    "gas_limit": {
                        "type": "string",
                        "description": "Optional gas limit"
                    },
                    "gas_price": {
                        "type": "string",
                        "description": "Optional gas price"
                    },
                    "master_password": {
                        "type": "string",
                        "description": "The master password to decrypt wallet private keys"
                    }
                },
                "required": ["wallet_name", "to_address", "amount", "chain_id", "master_password"]
            }),
        },
        Tool {
            name: "confirm_transaction".into(),
            description: "Confirm a pending transaction with the confirmation code".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction_id": {
                        "type": "string",
                        "description": "The transaction ID from the transfer request"
                    },
                    "confirmation_code": {
                        "type": "string",
                        "description": "The confirmation code shown in the transfer response"
                    },
                    "master_password": {
                        "type": "string",
                        "description": "The master password to decrypt wallet private keys"
                    }
                },
                "required": ["transaction_id", "confirmation_code", "master_password"]
            }),
        },
        Tool {
            name: "remove_wallet".into(),
            description: "Remove a registered wallet from storage".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "wallet_name": {
                        "type": "string",
                        "description": "The name of the wallet to remove"
                    },
                    "master_password": {
                        "type": "string",
                        "description": "The master password to decrypt wallet private keys"
                    }
                },
                "required": ["wallet_name", "master_password"]
            }),
        }
    ]
}
