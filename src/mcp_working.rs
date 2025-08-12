// src/mcp_working.rs

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::blockchain::client::SeiClient;
use crate::config::AppConfig;
use tracing::{debug, error, info};

// ... (keep all the struct definitions like JsonRpcRequest, etc., the same)
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
    let chars: String = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(6)
        .collect();
    chars.to_uppercase()
}

fn generate_transaction_id() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| rng.sample(Alphanumeric).to_string())
        .collect()
}

pub struct McpServer {
    client: SeiClient,
    config: AppConfig,
}

impl McpServer {
    pub fn new(config: AppConfig) -> Self {
        let client = SeiClient::new(&config.chain_rpc_urls, &config.websocket_url);
        Self { client, config }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting MCP server...");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut writer = stdout;
        let mut buffer = String::new();

        loop {
            match reader.read_line(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    // Skip empty lines
                    if buffer.trim().is_empty() {
                        buffer.clear();
                        continue;
                    }

                    debug!("Received raw message: {:?}", buffer.trim());

                    if let Some(response) = self.handle_message(&buffer).await {
                        let clean_response = response.trim();
                        writer.write_all(clean_response.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }
                    buffer.clear();
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_message(&self, message: &str) -> Option<String> {
        let message = message.trim();
        if message.is_empty() {
            return None;
        }

        match serde_json::from_str::<JsonRpcRequest>(message) {
            Ok(request) => {
                // *** CHANGE HERE: Check if the request is a notification ***
                // Notifications have an `id` of `null` or no `id` field at all.
                // The JSON-RPC spec says we MUST NOT reply to notifications.
                if request.id.is_none() {
                    self.handle_notification(request).await;
                    return None; // Do not send a response
                }

                // It's a regular request, so we process and get a response
                let response = self.handle_request(request).await;
                match serde_json::to_string(&response) {
                    Ok(json) => Some(json),
                    Err(e) => {
                        error!("Failed to serialize response: {}", e);
                        None
                    }
                }
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

    // *** NEW FUNCTION: To handle notifications without sending a response ***
    async fn handle_notification(&self, request: JsonRpcRequest) {
        match request.method.as_str() {
            "notifications/initialized" => {
                info!("Client initialized, ready for tool calls.");
                // We don't need to do anything else here.
            }
            _ => {
                info!("Received unhandled notification: {}", request.method);
            }
        }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            // We can remove the "notifications/initialized" case from here now
            "resources/list" | "prompts/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: Some(serde_json::json!({
                        "supported_methods": ["initialize", "tools/list", "tools/call"]
                    })),
                }),
            },
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
            protocol_version: "2025-06-18".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
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
    // ... (the rest of the file remains the same, no changes needed for tool handlers)
    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling tools/list request");

        let tools = vec![
            Tool {
                name: "get_balance".to_string(),
                description: Some("Get the balance of an address on a specific blockchain".to_string()),
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
                description: Some("Import a wallet from mnemonic phrase or private key".to_string()),
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
                description: Some("Get transaction history for an address (Sei chain only)".to_string()),
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
                            "description": "The amount to send in the smallest unit (e.g., usei)"
                        }
                    },
                    "required": ["chain_id", "from", "to", "amount"]
                }),
            },
            Tool {
                name: "transfer_sei".to_string(),
                description: Some("Transfer SEI tokens to another address (requires private key directly)".to_string()),
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
                            "description": "The amount of SEI to transfer in the smallest unit (e.g., usei)"
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
                description: Some("Register a wallet with encryption for persistent storage".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "wallet_name": {
                            "type": "string",
                            "description": "A unique name for the wallet"
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
                            "description": "The master password to access wallet storage"
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
                            "description": "The name of the registered wallet"
                        },
                        "chain_id": {
                            "type": "string",
                            "description": "The blockchain chain ID"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password for the wallet storage"
                        }
                    },
                    "required": ["wallet_name", "chain_id", "master_password"]
                }),
            },
            Tool {
                name: "transfer_from_wallet".to_string(),
                description: Some("Initiate a secure, two-step transfer from a registered wallet.".to_string()),
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
                            "description": "The amount to transfer in the smallest unit (e.g., usei)"
                        },
                        "chain_id": {
                            "type": "string",
                            "description": "The blockchain chain ID"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password for the wallet storage"
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
                description: Some("Confirm a pending transaction with the provided confirmation code.".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "transaction_id": {
                            "type": "string",
                            "description": "The transaction ID from the transfer initiation"
                        },
                        "confirmation_code": {
                            "type": "string",
                            "description": "The confirmation code from the transfer initiation"
                        },
                        "master_password": {
                            "type": "string",
                            "description": "The master password for the wallet storage"
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
                            "description": "The master password for the wallet storage"
                        }
                    },
                    "required": ["wallet_name", "master_password"]
                }),
            },
            Tool {
                name: "search_events".to_string(),
                description: Some("Search for past blockchain events based on various criteria like event type, attributes, and block range".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "event_type": {
                            "type": "string",
                            "description": "The type of event to search for (e.g., 'transfer', 'wasm')"
                        },
                        "attribute_key": {
                            "type": "string",
                            "description": "The attribute key to filter by (e.g., 'action', 'recipient')"
                        },
                        "attribute_value": {
                            "type": "string",
                            "description": "The attribute value to filter by (e.g., 'transfer', 'sei1abcd...')"
                        },
                        "from_block": {
                            "type": "integer",
                            "description": "Start block height for the search range"
                        },
                        "to_block": {
                            "type": "integer",
                            "description": "End block height for the search range"
                        },
                        "page": {
                            "type": "integer",
                            "description": "Page number for pagination (default: 1)"
                        },
                        "per_page": {
                            "type": "integer",
                            "description": "Number of results per page (default: 30, max: 100)"
                        }
                    }
                }),
            },
            Tool {
                name: "get_contract_events".to_string(),
                description: Some("Get events from a specific smart contract, optionally filtered by event type and block range".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "contract_address": {
                            "type": "string",
                            "description": "The address of the smart contract to get events from"
                        },
                        "event_type": {
                            "type": "string",
                            "description": "Optional event type to filter by (e.g., 'SwapExecuted')"
                        },
                        "from_block": {
                            "type": "integer",
                            "description": "Start block height for the search range"
                        },
                        "to_block": {
                            "type": "integer",
                            "description": "End block height for the search range"
                        },
                        "page": {
                            "type": "integer",
                            "description": "Page number for pagination (default: 1)"
                        },
                        "per_page": {
                            "type": "integer",
                            "description": "Number of results per page (default: 30, max: 100)"
                        }
                    },
                    "required": ["contract_address"]
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
        let request_id = request.id.clone();

        // Safely get the tool name for logging
        let tool_name_for_log = request
            .params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        info!(
            "Handling tools/call request for method: {}",
            tool_name_for_log
        );

        let params = request.params.unwrap_or(serde_json::json!({}));
        let call_request: CallToolRequest = match serde_json::from_value(params) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse tools/call request: {}", e);
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
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
            "search_events" => self.call_search_events(call_request.arguments).await,
            "get_contract_events" => self.call_get_contract_events(call_request.arguments).await,
            _ => {
                error!("Unknown tool: {}", call_request.name);
                let error_result = CallToolResult {
                    content: vec![Content::Text {
                        text: format!("Tool not found: {}", call_request.name),
                    }],
                    is_error: Some(true),
                };
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: Some(serde_json::to_value(error_result).unwrap()),
                    error: None,
                };
            }
        };

        match result {
            Ok(content) => {
                let call_result = CallToolResult {
                    content,
                    is_error: Some(false),
                };
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: Some(serde_json::to_value(call_result).unwrap()),
                    error: None,
                }
            }
            Err(e) => {
                error!("Tool execution error: {}", e);
                let error_result = CallToolResult {
                    content: vec![Content::Text {
                        text: e.to_string(),
                    }],
                    is_error: Some(true),
                };
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: Some(serde_json::to_value(error_result).unwrap()),
                    error: None,
                }
            }
        }
    }

    // --- Tool Implementations ---

    async fn call_get_balance(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let address = args["address"].as_str().context("Missing address")?;

        let balance = self.client.get_balance(chain_id, address).await?;
        let response = format!(
            "Balance for {}: {} {}",
            address, balance.amount, balance.denom
        );
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
        let args = arguments.context("Missing arguments")?;
        let key_or_mnemonic = args["mnemonic_or_private_key"]
            .as_str()
            .context("Missing mnemonic_or_private_key")?;

        let wallet = self.client.import_wallet(key_or_mnemonic).await?;
        let response = format!(
            "Wallet imported successfully!\nAddress: {}\nPrivate Key: {}",
            wallet.address, wallet.private_key
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_get_transaction_history(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let address = args["address"].as_str().context("Missing address")?;
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20);

        let history = self
            .client
            .get_transaction_history(chain_id, address, limit)
            .await?;
        let response = serde_json::to_string_pretty(&history.transactions)?;
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_estimate_fees(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let from = args["from"].as_str().context("Missing from address")?;
        let to = args["to"].as_str().context("Missing to address")?;
        let amount = args["amount"].as_str().context("Missing amount")?;

        let request = crate::blockchain::models::EstimateFeesRequest {
            from: from.to_string(),
            to: to.to_string(),
            amount: amount.to_string(),
        };

        let fees = self.client.estimate_fees(chain_id, &request).await?;
        let response = format!(
            "Estimated Fees:\n  Gas: {}\n  Gas Price: {}\n  Total Fee: {} {}",
            fees.estimated_gas, fees.gas_price, fees.total_fee, fees.denom
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_transfer_sei(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let to_address = args["to_address"].as_str().context("Missing to_address")?;
        let amount = args["amount"].as_str().context("Missing amount")?;
        let private_key = args["private_key"]
            .as_str()
            .context("Missing private_key")?;
        let gas_limit = args
            .get("gas_limit")
            .and_then(Value::as_str)
            .map(String::from);
        let gas_price = args
            .get("gas_price")
            .and_then(Value::as_str)
            .map(String::from);

        let request = crate::blockchain::models::SeiTransferRequest {
            to_address: to_address.to_string(),
            amount: amount.to_string(),
            private_key: private_key.to_string(),
            gas_limit,
            gas_price,
        };

        let result = self.client.transfer_sei(chain_id, &request).await?;
        let response = format!("Transfer successful! Transaction Hash: {}", result.tx_hash);
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_register_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let wallet_name = args["wallet_name"]
            .as_str()
            .context("Missing wallet_name")?;
        let private_key = args["private_key"]
            .as_str()
            .context("Missing private_key")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;

        // First, import the wallet to get the public address
        let wallet_info = self.client.import_wallet(private_key).await?;

        // Initialize wallet storage
        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;

        // Now, add the wallet to storage with the correct public address
        crate::mcp::wallet_storage::add_wallet_to_storage(
            wallet_name.to_string(),
            private_key.to_string(),
            wallet_info.address.clone(), // Use the derived address
            master_password,
        )?;

        let response = format!(
            "Wallet '{}' registered successfully! Address: {}",
            wallet_name, wallet_info.address
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_list_wallets(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;

        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;
        let wallets = crate::mcp::wallet_storage::list_wallets_from_storage()?;

        if wallets.is_empty() {
            return Ok(vec![Content::Text {
                text: "No wallets found.".to_string(),
            }]);
        }

        let wallet_list = wallets
            .iter()
            .map(|w| format!("- Name: {}, Address: {}", w.wallet_name, w.public_address))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(vec![Content::Text {
            text: format!("Registered Wallets:\n{}", wallet_list),
        }])
    }

    async fn call_get_wallet_balance(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let wallet_name = args["wallet_name"]
            .as_str()
            .context("Missing wallet_name")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;

        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;
        let wallet =
            crate::mcp::wallet_storage::get_wallet_from_storage(wallet_name, master_password)?;
        let balance = self
            .client
            .get_balance(chain_id, &wallet.public_address)
            .await?;

        let response = format!(
            "Balance for '{}' ({}): {} {}",
            wallet_name, wallet.public_address, balance.amount, balance.denom
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_transfer_from_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let wallet_name = args["wallet_name"]
            .as_str()
            .context("Missing wallet_name")?;
        let to_address = args["to_address"].as_str().context("Missing to_address")?;
        let amount = args["amount"].as_str().context("Missing amount")?;
        let chain_id = args["chain_id"].as_str().context("Missing chain_id")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;
        let gas_limit = args
            .get("gas_limit")
            .and_then(Value::as_str)
            .map(String::from);
        let gas_price = args
            .get("gas_price")
            .and_then(Value::as_str)
            .map(String::from);

        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;
        let _wallet =
            crate::mcp::wallet_storage::get_wallet_from_storage(wallet_name, master_password)?;

        let transaction_id = generate_transaction_id();
        let confirmation_code = generate_confirmation_code();

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

        PENDING_TRANSACTIONS
            .lock()
            .unwrap()
            .insert(transaction_id.clone(), pending_tx);

        let response = format!(
            "Transfer initiated. Please confirm with the following details:\n  Transaction ID: {}\n  Confirmation Code: {}",
            transaction_id, confirmation_code
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_confirm_transaction(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let transaction_id = args["transaction_id"]
            .as_str()
            .context("Missing transaction_id")?;
        let confirmation_code = args["confirmation_code"]
            .as_str()
            .context("Missing confirmation_code")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;

        let pending_tx = PENDING_TRANSACTIONS
            .lock()
            .unwrap()
            .remove(transaction_id)
            .context("Transaction not found or already processed.")?;

        if pending_tx.confirmation_code != confirmation_code {
            // Re-insert if code is wrong, so user can retry
            PENDING_TRANSACTIONS
                .lock()
                .unwrap()
                .insert(transaction_id.to_string(), pending_tx);
            return Err(anyhow!("Invalid confirmation code."));
        }

        if (Utc::now() - pending_tx.created_at).num_minutes() >= 5 {
            return Err(anyhow!("Transaction has expired. Please try again."));
        }

        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;
        let private_key = crate::mcp::wallet_storage::get_decrypted_private_key_from_storage(
            &pending_tx.wallet_name,
            master_password,
        )?;

        let request = crate::blockchain::models::SeiTransferRequest {
            to_address: pending_tx.to_address,
            amount: pending_tx.amount,
            private_key,
            gas_limit: pending_tx.gas_limit,
            gas_price: pending_tx.gas_price,
        };

        let result = self
            .client
            .transfer_sei(&pending_tx.chain_id, &request)
            .await?;
        let response = format!(
            "Transfer confirmed and sent! Transaction Hash: {}",
            result.tx_hash
        );
        Ok(vec![Content::Text { text: response }])
    }

    async fn call_remove_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let wallet_name = args["wallet_name"]
            .as_str()
            .context("Missing wallet_name")?;
        let master_password = args["master_password"]
            .as_str()
            .context("Missing master_password")?;

        crate::mcp::wallet_storage::initialize_wallet_storage(master_password)?;
        let removed = crate::mcp::wallet_storage::remove_wallet_from_storage(wallet_name)?;

        if removed {
            Ok(vec![Content::Text {
                text: format!("Wallet '{}' has been removed.", wallet_name),
            }])
        } else {
            Err(anyhow!("Wallet '{}' not found.", wallet_name))
        }
    }

    async fn call_search_events(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;

        let event_query = crate::blockchain::models::EventQuery {
            contract_address: None,
            event_type: args
                .get("event_type")
                .and_then(Value::as_str)
                .map(String::from),
            attribute_key: args
                .get("attribute_key")
                .and_then(Value::as_str)
                .map(String::from),
            attribute_value: args
                .get("attribute_value")
                .and_then(Value::as_str)
                .map(String::from),
            from_block: args.get("from_block").and_then(Value::as_u64),
            to_block: args.get("to_block").and_then(Value::as_u64),
        };

        let page = args.get("page").and_then(Value::as_u64).unwrap_or(1) as u32;
        let per_page = args.get("per_page").and_then(Value::as_u64).unwrap_or(30) as u8;

        let result = crate::blockchain::services::event::search_events(
            &self.client,
            event_query,
            page,
            per_page,
        )
        .await?;

        let response = format!(
            "Found {} events:\n{}",
            result.total_count,
            serde_json::to_string_pretty(&result.txs)?
        );

        Ok(vec![Content::Text { text: response }])
    }

    async fn call_get_contract_events(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args = arguments.context("Missing arguments")?;
        let contract_address = args["contract_address"]
            .as_str()
            .context("Missing contract_address")?;

        let event_query = crate::blockchain::models::EventQuery {
            contract_address: Some(contract_address.to_string()),
            event_type: args
                .get("event_type")
                .and_then(Value::as_str)
                .map(String::from),
            attribute_key: None,
            attribute_value: None,
            from_block: args.get("from_block").and_then(Value::as_u64),
            to_block: args.get("to_block").and_then(Value::as_u64),
        };

        let page = args.get("page").and_then(Value::as_u64).unwrap_or(1) as u32;
        let per_page = args.get("per_page").and_then(Value::as_u64).unwrap_or(30) as u8;

        let result = crate::blockchain::services::event::search_events(
            &self.client,
            event_query,
            page,
            per_page,
        )
        .await?;

        let response = format!(
            "Found {} events for contract {}:\n{}",
            result.total_count,
            contract_address,
            serde_json::to_string_pretty(&result.txs)?
        );

        Ok(vec![Content::Text { text: response }])
    }
}
