// src/mcp.rs
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

use crate::blockchain::client::SeiClient;
use crate::config::AppConfig;

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
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if let Some(response) = self.handle_message(&line).await {
                        writer.write_all(response.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }
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

        debug!("Received message: {}", message);

        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                return None;
            }
        };

        let response = self.handle_request(request).await;
        match serde_json::to_string(&response) {
            Ok(json) => Some(json),
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                None
            }
        }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            method => {
                error!("Unknown method: {}", method);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: "Method not found".to_string(),
                        data: None,
                    }),
                }
            }
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
                description: Some("Send SEI tokens to specified address".to_string()),
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
                            "description": "The amount of SEI tokens to send"
                        },
                        "private_key": {
                            "type": "string",
                            "description": "The sender's private key"
                        }
                    },
                    "required": ["chain_id", "to_address", "amount", "private_key"]
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

        let params: CallToolRequest = match request.params.as_ref() {
            Some(params) => match serde_json::from_value(params.clone()) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to parse tools/call params: {}", e);
                    return JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Invalid params".to_string(),
                            data: Some(serde_json::json!({"details": e.to_string()})),
                        }),
                    };
                }
            },
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let result = match params.name.as_str() {
            "get_balance" => self.call_get_balance(params.arguments).await,
            "create_wallet" => self.call_create_wallet(params.arguments).await,
            "import_wallet" => self.call_import_wallet(params.arguments).await,
            "get_transaction_history" => self.call_get_transaction_history(params.arguments).await,
            "estimate_fees" => self.call_estimate_fees(params.arguments).await,
            "transfer_sei" => self.call_transfer_sei(params.arguments).await,
            tool_name => {
                error!("Unknown tool: {}", tool_name);
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Unknown tool: {}", tool_name),
                        data: None,
                    }),
                };
            }
        };

        match result {
            Ok(content) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(
                    serde_json::to_value(CallToolResult {
                        content,
                        is_error: Some(false),
                    })
                    .unwrap(),
                ),
                error: None,
            },
            Err(e) => {
                error!("Tool call failed: {}", e);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(
                        serde_json::to_value(CallToolResult {
                            content: vec![Content::Text {
                                text: format!("Error: {}", e),
                            }],
                            is_error: Some(true),
                        })
                        .unwrap(),
                    ),
                    error: None,
                }
            }
        }
    }

    async fn call_get_balance(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args: serde_json::Map<String, Value> = arguments
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

        let address = args
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing address parameter"))?;

        match self.client.get_balance(chain_id, address).await {
            Ok(balance) => {
                let response = serde_json::to_string_pretty(&balance)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to get balance: {}", e)),
        }
    }

    async fn call_create_wallet(&self, _arguments: Option<Value>) -> Result<Vec<Content>> {
        match self.client.create_wallet().await {
            Ok(wallet) => {
                let response = serde_json::to_string_pretty(&wallet)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to create wallet: {}", e)),
        }
    }

    async fn call_import_wallet(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args: serde_json::Map<String, Value> = arguments
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let mnemonic_or_key = args
            .get("mnemonic_or_private_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing mnemonic_or_private_key parameter"))?;

        match self.client.import_wallet(mnemonic_or_key).await {
            Ok(wallet) => {
                let response = serde_json::to_string_pretty(&wallet)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to import wallet: {}", e)),
        }
    }

    async fn call_get_transaction_history(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args: serde_json::Map<String, Value> = arguments
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let chain_id = args
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing chain_id parameter"))?;

        let address = args
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing address parameter"))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);

        match self
            .client
            .get_transaction_history(chain_id, address, limit)
            .await
        {
            Ok(history) => {
                let response = serde_json::to_string_pretty(&history)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to get transaction history: {}", e)),
        }
    }

    async fn call_estimate_fees(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args: serde_json::Map<String, Value> = arguments
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

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

        match self.client.estimate_fees(chain_id, &request).await {
            Ok(fees) => {
                let response = serde_json::to_string_pretty(&fees)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to estimate fees: {}", e)),
        }
    }

    async fn call_transfer_sei(&self, arguments: Option<Value>) -> Result<Vec<Content>> {
        let args: serde_json::Map<String, Value> = arguments
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

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
            gas_limit: todo!(),
            gas_price: todo!(),
        };

        match self.client.transfer_sei(chain_id, &request).await {
            Ok(result) => {
                let response = serde_json::to_string_pretty(&result)?;
                Ok(vec![Content::Text { text: response }])
            }
            Err(e) => Err(anyhow!("Failed to transfer SEI tokens: {}", e)),
        }
    }
}
