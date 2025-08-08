pub mod enhanced_tools;
pub mod encryption;
pub mod protocol;
pub mod tools;
pub mod transport;
pub mod wallet_storage;

use protocol::*;
use tools::*;
use enhanced_tools::*;
use transport::run_loop;
use crate::blockchain::client::SeiClient;
use serde_json::{json, Value};
use anyhow::Result;

pub struct McpServer {
    client: SeiClient,
}

impl McpServer {
    pub fn new(client: SeiClient) -> Self {
        Self { client }
    }

    pub async fn run(&self) -> Result<()> {
        let client = self.client.clone();
        run_loop(move |msg| {
            let client = client.clone();
            let parsed: Result<JsonRpcRequest, _> = serde_json::from_str(&msg);
            if parsed.is_err() {
                return Some(error_response(Value::Null, -32700, "Parse error"));
            }
            let req = parsed.unwrap();

            match req.method.as_str() {
                "initialize" => {
                    Some(success_response(req.id, json!(InitializeResult {
                        protocol_version: "2024-11-05".to_string(),
                        capabilities: ServerCapabilities {
                            capabilities: Capabilities { tools: true }
                        },
                        server_info: ServerInfo {
                            name: "sei-mcp-server-rs".to_string(),
                            version: "0.1.0".to_string(),
                        },
                        instructions: Some("Sei blockchain MCP server for wallet operations, balance queries, and transaction management.".to_string()),
                    })))
                }
                "tools/list" => {
                    Some(success_response(req.id, json!(ListToolsResult {
                        tools: list_tools()
                    })))
                }
                "tools/call" => {
                    let params = req.params.unwrap_or(json!({}));
                    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = params.get("arguments").cloned();

                    // Use futures::executor::block_on for async operations
                    let result = futures::executor::block_on(dispatch_tool(&client, name, args));
                    match result {
                        Ok(val) => Some(success_response(req.id, val)),
                        Err(e) => Some(error_response(req.id, -32000, &e.to_string())),
                    }
                }
                _ => Some(error_response(req.id, -32601, "Method not found")),
            }
        }).await
    }
}

async fn dispatch_tool(client: &SeiClient, name: &str, args: Option<Value>) -> Result<Value> {
    match name {
        "get_balance" => call_get_balance(client, args).await,
        "create_wallet" => call_create_wallet(client, args).await,
        "import_wallet" => call_import_wallet(client, args).await,
        "get_transaction_history" => call_get_transaction_history(client, args).await,
        "estimate_fees" => call_estimate_fees(client, args).await,
        "transfer_sei" => call_transfer_sei(client, args).await,
        "register_wallet" => call_register_wallet(client, args).await,
        "list_wallets" => call_list_wallets(client, args).await,
        "get_wallet_balance" => call_get_wallet_balance(client, args).await,
        "transfer_from_wallet" => call_transfer_from_wallet(client, args).await,
        "confirm_transaction" => call_confirm_transaction(client, args).await,
        "remove_wallet" => call_remove_wallet(client, args).await,
        _ => Err(anyhow::anyhow!("Tool not found")),
    }
}

fn success_response(id: Value, result: Value) -> String {
    serde_json::to_string(&JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(result),
        error: None,
    }).unwrap()
}

fn error_response(id: Value, code: i32, msg: &str) -> String {
    serde_json::to_string(&JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message: msg.into() }),
    }).unwrap()
}
