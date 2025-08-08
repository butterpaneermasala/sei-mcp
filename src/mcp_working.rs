// src/mcp_working.rs
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

use crate::blockchain::client::SeiClient;
use crate::blockchain::models::{ClaimRewardsRequest, StakeRequest, UnstakeRequest};
use crate::config::AppConfig;
use crate::mcp::encryption::EncryptionManager;
use crate::mcp::wallet_storage::{StoredWallet, WalletStorage};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Mutex;

// JSON-RPC message structures
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// MCP-specific structures
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
}

// Global pending transactions storage
lazy_static! {
    static ref PENDING_TRANSACTIONS: Mutex<HashMap<String, PendingTransaction>> =
        Mutex::new(HashMap::new());
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
    let mut rng = rand::thread_rng();
    let letters: String = (0..3)
        .map(|_| rng.sample(Alphanumeric) as char)
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let numbers: String = (0..3)
        .map(|_| rng.sample(Alphanumeric) as char)
        .filter(|c| c.is_ascii_digit())
        .collect();
    format!("{}{}", letters, numbers)
}

fn generate_transaction_id() -> String {
    let mut rng = rand::thread_rng();
    (0..6).map(|_| rng.sample(Alphanumeric) as char).collect()
}

pub struct McpServer {
    client: SeiClient,
    config: AppConfig,
}

impl McpServer {
    pub fn new(config: AppConfig) -> Self {
        let client = SeiClient::new(&config.chain_rpc_urls);
        Self { client, config }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting MCP server...");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut writer = stdout;

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await? == 0 {
                break;
            }

            if let Some(response) = self.handle_message(&line).await {
                writer.write_all(response.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }

    async fn handle_message(&self, message: &str) -> Option<String> {
        let message = message.trim();
        if message.is_empty() {
            return None;
        }

        debug!("Received message: {}", message);

        match serde_json::from_str::<JsonRpcRequest>(message) {
            Ok(request) => {
                let response = self.handle_request(request).await;
                Some(serde_json::to_string(&response).unwrap())
            }
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: "Parse error".to_string(),
                        data: None,
                    }),
                };
                Some(serde_json::to_string(&error_response).unwrap())
            }
        }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }),
            },
        }
    }

    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling initialize request");

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                resources: None,
                prompts: None,
            },
            server_info: ServerInfo {
                name: "sei-mcp-server-rs".to_string(),
                version: "0.1.0".to_string(),
            },
            instructions: Some("Sei blockchain MCP server for wallet operations, balance queries, and transaction management.".to_string()),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling tools/list request");

        let tools = vec![
            Tool {
                name: "get_balance".to_string(),
                description: Some(
                    "Get the balance of an address on a specific blockchain".to_string(),
                ),
                input_schema: serde_json::json!({
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
                name: "create_wallet".to_string(),
                description: Some("Create a new wallet with mnemonic phrase".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "import_wallet".to_string(),
                description: Some(
                    "Import a wallet from mnemonic phrase or private key".to_string(),
                ),
                input_schema: serde_json::json!({
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
                name: "get_transaction_history".to_string(),
                description: Some(
                    "Get transaction history for an address (Sei chain only)".to_string(),
                ),
                input_schema: serde_json::json!({
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
                name: "estimate_fees".to_string(),
                description: Some("Estimate transaction fees for a transfer".to_string()),
                input_schema: serde_json::json!({
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
                name: "transfer_sei".to_string(),
                description: Some("Transfer SEI tokens to another address".to_string()),
                input_schema: serde_json::json!({
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
            },
            Tool {
                name: "register_wallet".to_string(),
                description: Some(
                    "Register a wallet with encryption for persistent storage".to_string(),
                ),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "wallet_name": {
                            "type": "string",
                            "description": "The name for the wallet"
                        },
                        "private_key": {
                            "type": "string",
                            "description": "The private key to register"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password for encryption"
                        }
                    },
                    "required": ["wallet_name", "private_key", "master_password"]
                }),
            },
            Tool {
                name: "list_wallets".to_string(),
                description: Some("List all registered wallets".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "master_password": {
                            "type": "string",
                            "description": "The master password to decrypt wallets"
                        }
                    },
                    "required": ["master_password"]
                }),
            },
            Tool {
                name: "get_wallet_balance".to_string(),
                description: Some("Get balance of a registered wallet".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "wallet_name": {
                            "type": "string",
                            "description": "The name of the wallet"
                        },
                        "chain_id": {
                            "type": "string",
                            "description": "The blockchain chain ID"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password to decrypt wallet"
                        }
                    },
                    "required": ["wallet_name", "chain_id", "master_password"]
                }),
            },
            Tool {
                name: "transfer_from_wallet".to_string(),
                description: Some(
                    "Initiate transfer from a registered wallet (two-step process)".to_string(),
                ),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "wallet_name": {
                            "type": "string",
                            "description": "The name of the wallet to transfer from"
                        },
                        "to_address": {
                            "type": "string",
                            "description": "The recipient address"
                        },
                        "amount": {
                            "type": "string",
                            "description": "The amount to transfer"
                        },
                        "chain_id": {
                            "type": "string",
                            "description": "The blockchain chain ID"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password to decrypt wallet"
                        },
                        "gas_limit": {
                            "type": "string",
                            "description": "Optional gas limit"
                        },
                        "gas_price": {
                            "type": "string",
                            "description": "Optional gas price"
                        }
                    },
                    "required": ["wallet_name", "to_address", "amount", "chain_id", "master_password"]
                }),
            },
            Tool {
                name: "confirm_transaction".to_string(),
                description: Some(
                    "Confirm a pending transaction with confirmation code".to_string(),
                ),
                input_schema: serde_json::json!({
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
                            "description": "The master password to decrypt wallet"
                        }
                    },
                    "required": ["transaction_id", "confirmation_code", "master_password"]
                }),
            },
            Tool {
                name: "remove_wallet".to_string(),
                description: Some("Remove a registered wallet from storage".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "wallet_name": {
                            "type": "string",
                            "description": "The name of the wallet to remove"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password to decrypt wallet"
                        }
                    },
                    "required": ["wallet_name", "master_password"]
                }),
            },
            Tool {
                name: "stake_tokens".to_string(),
                description: Some("Stake (delegate) tokens to a validator.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": { "type": "string", "description": "e.g., 'sei' or 'sei-testnet'" },
                        "validator_address": { "type": "string" },
                        "amount": { "type": "string", "description": "Amount in usei" },
                        "private_key": { "type": "string" },
                        "gas_fee": { "type": "integer", "description": "Gas fee in usei, e.g., 7500" }
                    },
                    "required": ["chain_id", "validator_address", "amount", "private_key", "gas_fee"]
                }),
            },
            Tool {
                name: "unstake_tokens".to_string(),
                description: Some("Unstake (unbond) tokens from a validator.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": { "type": "string" },
                        "validator_address": { "type": "string" },
                        "amount": { "type": "string", "description": "Amount in usei" },
                        "private_key": { "type": "string" },
                        "gas_fee": { "type": "integer", "description": "Gas fee in usei, e.g., 7500" }
                    },
                    "required": ["chain_id", "validator_address", "amount", "private_key", "gas_fee"]
                }),
            },
            Tool {
                name: "claim_rewards".to_string(),
                description: Some("Claim staking rewards from a validator.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": { "type": "string" },
                        "validator_address": { "type": "string" },
                        "private_key": { "type": "string" },
                        "gas_fee": { "type": "integer", "description": "Gas fee in usei, e.g., 7500" }
                    },
                    "required": ["chain_id", "validator_address", "private_key", "gas_fee"]
                }),
            },
            Tool {
                name: "get_validators".to_string(),
                description: Some("Get a list of all validators and their info.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": { "type": "string" }
                    },
                    "required": ["chain_id"]
                }),
            },
            Tool {
                name: "get_staking_apr".to_string(),
                description: Some("Get the current estimated staking APR.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": { "type": "string" }
                    },
                    "required": ["chain_id"]
                }),
            },
        ];

        let result = ListToolsResult { tools };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling tools/call request");

        let params = request.params.unwrap_or(serde_json::json!({}));
        let call_request: CallToolRequest = match serde_json::from_value(params) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse tools/call request: {}", e);
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let result = match call_request.name.as_str() {
            "get_balance" => self.call_get_balance(call_request.arguments).await,
            "create_wallet" => self.call_create_wallet(call_request.arguments).await,
            "import_wallet" => self.call_import_wallet(call_request.arguments).await,
            "get_transaction_history" => {
                self.call_get_transaction_history(call_request.arguments)
                    .await
            }
            "estimate_fees" => self.call_estimate_fees(call_request.arguments).await,
            "transfer_sei" => self.call_transfer_sei(call_request.arguments).await,
            "register_wallet" => self.call_register_wallet(call_request.arguments).await,
            "list_wallets" => self.call_list_wallets(call_request.arguments).await,
            "get_wallet_balance" => self.call_get_wallet_balance(call_request.arguments).await,
            "transfer_from_wallet" => self.call_transfer_from_wallet(call_request.arguments).await,
            "confirm_transaction" => self.call_confirm_transaction(call_request.arguments).await,
            "remove_wallet" => self.call_remove_wallet(call_request.arguments).await,
            "stake_tokens" => self.call_stake_tokens(call_request.arguments).await,
            "unstake_tokens" => self.call_unstake_tokens(call_request.arguments).await,
            "claim_rewards" => self.call_claim_rewards(call_request.arguments).await,
            "get_validators" => self.call_get_validators(call_request.arguments).await,
            "get_staking_apr" => self.call_get_staking_apr(call_request.arguments).await,
            _ => {
                error!("Unknown tool: {}", call_request.name);
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Tool not found: {}", call_request.name),
                        data: None,
                    }),
                };
            }
        };

        match result {
            Ok(content) => {
                let call_result = CallToolResult {
                    content,
                    is_error: None,
                };

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serde_json::to_value(call_result).unwrap()),
                    error: None,
                }
            }
            Err(e) => {
                error!("Tool execution error: {}", e);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32000,
                        message: e.to_string(),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn call_get_balance(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let address = args
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing address parameter"))?;

        let balance = self.client.get_balance(chain_id, address).await?;
        let response = format!("Balance: {} {}", balance.amount, balance.denom);

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_create_wallet(&self, _arguments: Option<Value>) -> Result<Vec<Content>> {
        let wallet = self.client.create_wallet().await?;
        let response = format!(
            "Wallet created successfully!\nAddress: {}\nPrivate Key: {}\nMnemonic: {}",
            wallet.address,
            wallet.private_key,
            wallet.mnemonic.unwrap_or_default()
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_import_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let mnemonic_or_key = args
            .get("mnemonic_or_private_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing mnemonic_or_private_key parameter"))?;

        let wallet = self.client.import_wallet(mnemonic_or_key).await?;
        let response = format!(
            "Wallet imported successfully!\nAddress: {}\nPrivate Key: {}\nMnemonic: {}",
            wallet.address,
            wallet.private_key,
            wallet.mnemonic.unwrap_or_default()
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_get_transaction_history(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let address = args
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing address parameter"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);

        let history = self
            .client
            .get_transaction_history(chain_id, address, limit)
            .await?;
        let response = format!(
            "Transaction history for {} ({} transactions):\n{}",
            address,
            history.transactions.len(),
            serde_json::to_string_pretty(&history.transactions).unwrap()
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_estimate_fees(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let from = args
            .get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing from parameter"))?;
        let to = args
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing to parameter"))?;
        let amount = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing amount parameter"))?;

        let request = crate::blockchain::models::EstimateFeesRequest {
            from: from.to_string(),
            to: to.to_string(),
            amount: amount.to_string(),
        };

        let fees = self.client.estimate_fees(chain_id, &request).await?;
        let response = format!(
            "Fee estimation:\nEstimated Gas: {}\nGas Price: {}\nTotal Fee: {} {}\nDenom: {}",
            fees.estimated_gas, fees.gas_price, fees.total_fee, fees.denom, fees.denom
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_transfer_sei(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let to_address = args
            .get("to_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing to_address parameter"))?;
        let amount = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing amount parameter"))?;
        let private_key = args
            .get("private_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing private_key parameter"))?;

        let request = crate::blockchain::models::SeiTransferRequest {
            to_address: to_address.to_string(),
            amount: amount.to_string(),
            private_key: private_key.to_string(),
            gas_limit: None,
            gas_price: None,
        };

        let result = self.client.transfer_sei(chain_id, &request).await?;
        let response = format!(
            "Transfer completed successfully!\nTransaction Hash: {}",
            result.tx_hash
        );

        Ok(vec![Content::Text { text: response }])
    }

    // Enhanced wallet methods (Cast-like features)
    async fn call_register_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let wallet_name = args
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;
        let private_key = args
            .get("private_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing private_key parameter"))?;
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // Add wallet to storage
        crate::mcp::wallet_storage::add_wallet_to_storage(
            wallet_name.to_string(),
            private_key.to_string(),
            "".to_string(), // We'll get the address from the private key
            master_password,
        )?;

        let response = format!(
            "Wallet '{}' registered successfully with encryption!",
            wallet_name
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_list_wallets(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // List wallets from storage
        let wallets = crate::mcp::wallet_storage::list_wallets_from_storage()?;

        if wallets.is_empty() {
            let response = "No wallets found. Register a wallet first using register_wallet.";
            Ok(vec![Content::Text {
                text: response.to_string(),
            }])
        } else {
            let response = format!(
                "Registered wallets ({}):\n{}",
                wallets.len(),
                wallets
                    .iter()
                    .map(|w| format!("- {}: {}", w.wallet_name, w.public_address))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            Ok(vec![Content::Text { text: response }])
        }
    }

    async fn call_get_wallet_balance(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let wallet_name = args
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // Get wallet from storage
        let wallet =
            crate::mcp::wallet_storage::get_wallet_from_storage(wallet_name, master_password)?;

        // Get balance using the wallet's address
        let balance = self
            .client
            .get_balance(chain_id, &wallet.public_address)
            .await?;
        let response = format!(
            "Wallet '{}' balance: {} {}",
            wallet_name, balance.amount, balance.denom
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_transfer_from_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let wallet_name = args
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;
        let to_address = args
            .get("to_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing to_address parameter"))?;
        let amount = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing amount parameter"))?;
        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;
        let gas_limit = args
            .get("gas_limit")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let gas_price = args
            .get("gas_price")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // Get wallet from storage
        let wallet =
            crate::mcp::wallet_storage::get_wallet_from_storage(wallet_name, master_password)?;

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

        {
            let mut pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
            pending_storage.insert(transaction_id.clone(), pending_tx);
        }

        let response = format!(
            "Transfer initiated!\nTransaction ID: {}\nConfirmation Code: {}\n\nTo confirm this transfer, use the confirm_transaction tool with the above details.",
            transaction_id, confirmation_code
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_confirm_transaction(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let transaction_id = args
            .get("transaction_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing transaction_id parameter"))?;
        let confirmation_code = args
            .get("confirmation_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing confirmation_code parameter"))?;
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

        // Get pending transaction
        let pending_tx = {
            let pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
            pending_storage
                .get(transaction_id)
                .cloned()
                .ok_or_else(|| anyhow!("Transaction not found or expired"))?
        };

        // Verify confirmation code
        if pending_tx.confirmation_code != confirmation_code {
            return Err(anyhow!("Invalid confirmation code"));
        }

        // Check if transaction is expired (5 minutes)
        let now = Utc::now();
        if (now - pending_tx.created_at).num_minutes() > 5 {
            // Remove expired transaction
            {
                let mut pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
                pending_storage.remove(transaction_id);
            }
            return Err(anyhow!(
                "Transaction expired. Please initiate a new transfer."
            ));
        }

        // Get wallet from storage
        let wallet = crate::mcp::wallet_storage::get_wallet_from_storage(
            &pending_tx.wallet_name,
            master_password,
        )?;

        // Execute transfer
        let request = crate::blockchain::models::SeiTransferRequest {
            to_address: pending_tx.to_address.clone(),
            amount: pending_tx.amount.clone(),
            private_key: crate::mcp::wallet_storage::get_decrypted_private_key_from_storage(
                &pending_tx.wallet_name,
                master_password,
            )?,
            gas_limit: pending_tx.gas_limit.clone(),
            gas_price: pending_tx.gas_price.clone(),
        };

        let result = self
            .client
            .transfer_sei(&pending_tx.chain_id, &request)
            .await?;

        // Remove pending transaction
        {
            let mut pending_storage = PENDING_TRANSACTIONS.lock().unwrap();
            pending_storage.remove(transaction_id);
        }

        let response = format!(
            "Transfer confirmed successfully!\nTransaction Hash: {}\nFrom: {}\nTo: {}\nAmount: {}",
            result.tx_hash, pending_tx.wallet_name, pending_tx.to_address, pending_tx.amount
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_remove_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or(serde_json::json!({}));
        let wallet_name = args
            .get("wallet_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing wallet_name parameter"))?;
        let master_password = args
            .get("master_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing master_password parameter"))?;

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // Remove wallet from storage
        let removed = crate::mcp::wallet_storage::remove_wallet_from_storage(wallet_name)?;

        if removed {
            let response = format!(
                "Wallet '{}' removed successfully from storage.",
                wallet_name
            );
            Ok(vec![Content::Text { text: response }])
        } else {
            Err(anyhow!("Wallet '{}' not found", wallet_name))
        }
    }
    async fn call_stake_tokens(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or_default();
        let chain_id = args["chain_id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing chain_id"))?;
        let request: StakeRequest = serde_json::from_value(args.clone())?;

        let result = self.client.stake_tokens(chain_id, &request).await?;
        let response = format!("Staking transaction sent! Hash: {}", result.tx_hash);
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_unstake_tokens(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or_default();
        let chain_id = args["chain_id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing chain_id"))?;
        let request: UnstakeRequest = serde_json::from_value(args.clone())?;

        let result = self.client.unstake_tokens(chain_id, &request).await?;
        let response = format!("Unstaking transaction sent! Hash: {}", result.tx_hash);
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_claim_rewards(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or_default();
        let chain_id = args["chain_id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing chain_id"))?;
        let request: ClaimRewardsRequest = serde_json::from_value(args.clone())?;

        let result = self.client.claim_rewards(chain_id, &request).await?;
        let response = format!("Claim rewards transaction sent! Hash: {}", result.tx_hash);
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_get_validators(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or_default();
        let chain_id = args["chain_id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing chain_id"))?;

        let validators = self.client.get_all_validators(chain_id).await?;
        let response = format!(
            "Validators:\n{}",
            serde_json::to_string_pretty(&validators)?
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_get_staking_apr(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.unwrap_or_default();
        let chain_id = args["chain_id"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing chain_id"))?;

        let apr = self.client.get_staking_apr(chain_id).await?;
        let response = format!("Estimated Staking APR for {}: {}%", chain_id, apr);
        Ok(vec![Content::Text { text: response }])
    }
}
