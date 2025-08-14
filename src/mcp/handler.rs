// src/mcp/handler.rs

use crate::blockchain::client::SeiClient;
use crate::mcp::protocol::{error_codes, Request, Response};
use crate::blockchain::models::{
    EstimateFeesRequest, EventQuery, ImportWalletRequest, SeiTransferRequest,
};
use crate::blockchain::services::wallet;
use crate::config::Config;
use crate::mcp::wallet_storage::{
    get_wallet_storage_path, WalletStorage,
};
use serde::Deserialize;
use serde_json::{from_value, json};
use tracing::info;

// --- Helper structs for deserializing tool arguments ---

#[derive(Deserialize)]
struct SearchEventsArgs {
    event_type: Option<String>,
    attribute_key: Option<String>,
    attribute_value: Option<String>,
    from_block: Option<u64>,
    to_block: Option<u64>,
    page: Option<u32>,
    per_page: Option<u8>,
}

#[derive(Deserialize)]
struct ContractEventsArgs {
    contract_address: String,
    event_type: Option<String>,
    from_block: Option<u64>,
    to_block: Option<u64>,
    page: Option<u32>,
    per_page: Option<u8>,
}


/// This is the main dispatcher for all incoming MCP requests.
/// It returns an Option<Response> because notifications should not get a response.
pub async fn handle_mcp_request(req: Request, config: &Config) -> Option<Response> {
    info!("Handling MCP request: {}", req.method);

    if req.is_notification() {
        return None;
    }

    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "tools/list" => handle_tools_list(&req),
        "tools/call" => handle_tools_call(&req, config).await,
        _ => Response::error(
            req.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", req.method),
        ),
    };

    Some(response)
}
/// Handles the 'initialize' request.
fn handle_initialize(req: &Request) -> Response {
    let server_info = json!({
        "name": "sei-mcp-server-rs",
        "version": "0.1.0"
    });
    let capabilities = json!({ "tools": { "listChanged": false } });
    let instructions =
        "Sei blockchain MCP server for wallet operations, balance queries, and transaction management.";

    Response::success(
        req.id.clone(),
        json!({
            "serverInfo": server_info,
            "protocolVersion": "2025-06-18",
            "capabilities": capabilities,
            "instructions": instructions
        }),
    )
}

/// Handles the 'tools/list' request by returning a JSON definition of all available tools.
fn handle_tools_list(req: &Request) -> Response {
    let tools = json!([
        {
            "name": "get_balance",
            "description": "Get the balance of an address on a specific blockchain",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "The blockchain chain ID (e.g., 'sei-testnet')"},
                    "address": {"type": "string", "description": "The wallet address to check balance for"}
                },
                "required": ["chain_id", "address"]
            }
        },
        {
            "name": "create_wallet",
            "description": "Create a new wallet with mnemonic phrase",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "import_wallet",
            "description": "Import a wallet from mnemonic phrase or private key",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mnemonic_or_private_key": {"type": "string", "description": "The mnemonic phrase or private key to import"}
                },
                "required": ["mnemonic_or_private_key"]
            }
        },
        {
            "name": "get_transaction_history",
            "description": "Get transaction history for an address (Sei chains only)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "The blockchain chain ID (e.g., 'sei-testnet')"},
                    "address": {"type": "string", "description": "The wallet address to get history for"},
                    "limit": {"type": "integer", "description": "Number of transactions to return (default: 20, max: 100)", "minimum": 1, "maximum": 100}
                },
                "required": ["chain_id", "address"]
            }
        },
        {
            "name": "estimate_fees",
            "description": "Estimate transaction fees for a transfer",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "The blockchain chain ID"},
                    "from": {"type": "string", "description": "The sender address"},
                    "to": {"type": "string", "description": "The recipient address"},
                    "amount": {"type": "string", "description": "The amount to send in the smallest unit (e.g., usei)"}
                },
                "required": ["chain_id", "from", "to", "amount"]
            }
        },
        {
            "name": "transfer_sei",
            "description": "Transfer SEI tokens to another address (requires private key directly)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "The blockchain chain ID"},
                    "to_address": {"type": "string", "description": "The recipient address"},
                    "amount": {"type": "string", "description": "The amount of SEI to transfer in the smallest unit (e.g., usei)"},
                    "private_key": {"type": "string", "description": "The private key of the sender wallet"},
                    "gas_limit": {"type": "string", "description": "Optional gas limit (default: 100000)"},
                    "gas_price": {"type": "string", "description": "Optional gas price (default: 20000000000)"}
                },
                "required": ["chain_id", "to_address", "amount", "private_key"]
            }
        },
        {
            "name": "request_faucet",
            "description": "Request testnet tokens from the faucet for an EVM address.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": {"type": "string", "description": "The EVM (0x...) address to receive tokens"}
                },
                "required": ["address"]
            }
        },
        {
            "name": "search_events",
            "description": "Search for past blockchain events based on various criteria like event type, attributes, and block range",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "event_type": {"type": "string", "description": "The type of event to search for (e.g., 'transfer', 'wasm')"},
                    "attribute_key": {"type": "string", "description": "The attribute key to filter by (e.g., 'action', 'recipient')"},
                    "attribute_value": {"type": "string", "description": "The attribute value to filter by (e.g., 'transfer', 'sei1abcd...')"},
                    "from_block": {"type": "integer", "description": "Start block height for the search range"},
                    "to_block": {"type": "integer", "description": "End block height for the search range"},
                    "page": {"type": "integer", "description": "Page number for pagination (default: 1)"},
                    "per_page": {"type": "integer", "description": "Number of results per page (default: 30, max: 100)"}
                }
            }
        },
        {
            "name": "get_contract_events",
            "description": "Get events from a specific smart contract, optionally filtered by event type and block range",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_address": {"type": "string", "description": "The address of the smart contract to get events from"},
                    "event_type": {"type": "string", "description": "Optional event type to filter by (e.g., 'SwapExecuted')"},
                    "from_block": {"type": "integer", "description": "Start block height for the search range"},
                    "to_block": {"type": "integer", "description": "End block height for the search range"},
                    "page": {"type": "integer", "description": "Page number for pagination (default: 1)"},
                    "per_page": {"type": "integer", "description": "Number of results per page (default: 30, max: 100)"}
                },
                "required": ["contract_address"]
            }
        },
        {
            "name": "register_wallet",
            "description": "Register a wallet with encryption for persistent storage",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "A unique name for the wallet"},
                    "private_key": {"type": "string", "description": "The private key to register"},
                    "master_password": {"type": "string", "description": "The master password for encryption"}
                },
                "required": ["wallet_name", "private_key", "master_password"]
            }
        },
        {
            "name": "list_wallets",
            "description": "List all registered wallets",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "master_password": {"type": "string", "description": "The master password to access wallet storage"}
                },
                "required": ["master_password"]
            }
        },
        {
            "name": "get_wallet_balance",
            "description": "Get balance of a registered wallet",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "The name of the registered wallet"},
                    "chain_id": {"type": "string", "description": "The blockchain chain ID"},
                    "master_password": {"type": "string", "description": "The master password for the wallet storage"}
                },
                "required": ["wallet_name", "chain_id", "master_password"]
            }
        },
        {
            "name": "transfer_from_wallet",
            "description": "Transfer from a registered wallet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "The name of the wallet to transfer from"},
                    "to_address": {"type": "string", "description": "The recipient address"},
                    "amount": {"type": "string", "description": "The amount to transfer in the smallest unit (e.g., usei)"},
                    "chain_id": {"type": "string", "description": "The blockchain chain ID"},
                    "master_password": {"type": "string", "description": "The master password for the wallet storage"},
                    "gas_limit": {"type": "string", "description": "Optional gas limit"},
                    "gas_price": {"type": "string", "description": "Optional gas price"}
                },
                "required": ["wallet_name", "to_address", "amount", "chain_id", "master_password"]
            }
        },
        {
            "name": "remove_wallet",
            "description": "Remove a registered wallet from storage",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "The name of the wallet to remove"},
                    "master_password": {"type": "string", "description": "The master password for the wallet storage"}
                },
                "required": ["wallet_name", "master_password"]
            }
        }
    ]);
    Response::success(req.id.clone(), json!({ "tools": tools }))
}

/// Handles the 'tools/call' request by dispatching to the correct tool function.
async fn handle_tools_call(req: &Request, config: &Config) -> Response {
    let params = match req.params.as_ref() {
        Some(p) => p,
        None => return Response::error(req.id.clone(), -32602, "Invalid params: missing".into()),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => return Response::error(req.id.clone(), -32602, "Invalid params: missing tool name".into()),
    };

    let args = match params.get("arguments") {
        Some(args) => args.clone(),
        None => json!({}),
    };

    let client = SeiClient::new(&config.chain_rpc_urls, &config.websocket_url);

    match tool_name {
        "get_balance" => {
            let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
            let address: String = from_value(args["address"].clone()).unwrap_or_default();
            match client.get_balance(&chain_id, &address).await {
                Ok(balance) => Response::success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("Balance for {}: {} {}", address, balance.amount, balance.denom) }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to get balance: {}", e)),
            }
        }
        "create_wallet" => {
            match client.create_wallet().await {
                Ok(wallet) => Response::success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("New Wallet Created:\nAddress: {}\nMnemonic: {}", wallet.address, wallet.mnemonic.unwrap_or_default()) }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to create wallet: {}", e)),
            }
        }
        "import_wallet" => {
            let input: ImportWalletRequest = match from_value(args) {
                Ok(val) => val,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments: {}", e)),
            };
            match client.import_wallet(&input.mnemonic_or_private_key).await {
                Ok(wallet) => Response::success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("Wallet Imported:\nAddress: {}", wallet.address) }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to import wallet: {}", e)),
            }
        }
         "get_transaction_history" => {
            let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
            let address: String = from_value(args["address"].clone()).unwrap_or_default();
            let limit: u64 = from_value(args["limit"].clone()).unwrap_or(20);
            match client.get_transaction_history(&chain_id, &address, limit).await {
                Ok(history) => Response::success(req.id.clone(), json!({ "content": [{ "type": "json", "json": history }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to get history: {}", e)),
            }
        }
        "estimate_fees" => {
            let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
            let request: EstimateFeesRequest = match from_value(args) {
                Ok(val) => val,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments: {}", e)),
            };
            match client.estimate_fees(&chain_id, &request).await {
                Ok(fees) => Response::success(req.id.clone(), json!({ "content": [{ "type": "json", "json": fees }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to estimate fees: {}", e)),
            }
        }
        "transfer_sei" => {
            let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
            let request: SeiTransferRequest = match from_value(args) {
                Ok(val) => val,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments: {}", e)),
            };
            match client.transfer_sei(&chain_id, &request).await {
                Ok(resp) => Response::success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("Transaction sent: {}", resp.tx_hash) }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Transfer failed: {}", e)),
            }
        }
        "request_faucet" => {
            let address: String = match from_value(args["address"].clone()) {
                Ok(addr) => addr,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments: missing address field: {}", e)),
            };

            if !address.starts_with("0x") {
                return Response::error(req.id.clone(), -32602, "Invalid address format. Only EVM (0x...) addresses are supported.".into());
            }

            match crate::blockchain::services::faucet::send_faucet_tokens(config, &address).await {
                Ok(tx_hash) => Response::success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("Faucet request successful. Tx Hash: {}", tx_hash) }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Faucet request failed: {}", e)),
            }
        }
        "search_events" => {
            let search_args: SearchEventsArgs = match from_value(args) {
                Ok(val) => val,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments for search_events: {}", e)),
            };
            let event_query = EventQuery {
                contract_address: None,
                event_type: search_args.event_type,
                attribute_key: search_args.attribute_key,
                attribute_value: search_args.attribute_value,
                from_block: search_args.from_block,
                to_block: search_args.to_block,
            };
            let page = search_args.page.unwrap_or(1);
            let per_page = search_args.per_page.unwrap_or(30);

            match crate::blockchain::services::event::search_events(&client, event_query, page, per_page).await {
                Ok(result) => Response::success(req.id.clone(), json!({ "content": [{ "type": "json", "json": result }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to search events: {}", e)),
            }
        }
        "get_contract_events" => {
            let contract_args: ContractEventsArgs = match from_value(args) {
                Ok(val) => val,
                Err(e) => return Response::error(req.id.clone(), -32602, format!("Invalid arguments for get_contract_events: {}", e)),
            };
            let event_query = EventQuery {
                contract_address: Some(contract_args.contract_address),
                event_type: contract_args.event_type,
                attribute_key: None,
                attribute_value: None,
                from_block: contract_args.from_block,
                to_block: contract_args.to_block,
            };
            let page = contract_args.page.unwrap_or(1);
            let per_page = contract_args.per_page.unwrap_or(30);

            match crate::blockchain::services::event::search_events(&client, event_query, page, per_page).await {
                Ok(result) => Response::success(req.id.clone(), json!({ "content": [{ "type": "json", "json": result }]})),
                Err(e) => Response::error(req.id.clone(), -32000, format!("Failed to get contract events: {}", e)),
            }
        }
        "register_wallet" | "list_wallets" | "get_wallet_balance" | "transfer_from_wallet" | "remove_wallet" => {
            let master_password: String = match from_value(args["master_password"].clone()) {
                Ok(pass) => pass,
                Err(_) => return Response::error(req.id.clone(), -32602, "Missing 'master_password' argument".into()),
            };
            let storage_path = match get_wallet_storage_path() {
                Ok(path) => path,
                Err(e) => return Response::error(req.id.clone(), -32000, format!("Storage error: {}", e)),
            };

            let mut storage = match WalletStorage::load_from_file(&storage_path, &master_password) {
                Ok(s) => s,
                Err(e) => return Response::error(req.id.clone(), -32000, format!("Failed to load wallet storage: {}", e)),
            };

            let result = match tool_name {
                "register_wallet" => {
                    let wallet_name: String = from_value(args["wallet_name"].clone()).unwrap_or_default();
                    let private_key: String = from_value(args["private_key"].clone()).unwrap_or_default();
                    
                    let wallet_info = match wallet::import_wallet(&private_key) {
                        Ok(info) => info,
                        Err(e) => return Response::error(req.id.clone(), -32000, format!("Invalid private key: {}", e)),
                    };

                    match storage.add_wallet(wallet_name.clone(), private_key, wallet_info.address, &master_password) {
                        Ok(_) => Ok(json!({ "content": [{ "type": "text", "text": format!("Wallet '{}' registered successfully.", wallet_name) }]})),
                        Err(e) => Err(format!("Failed to register wallet: {}", e)),
                    }
                }
                "list_wallets" => {
                    let wallets = storage.list_wallets();
                    Ok(json!({ "content": [{ "type": "json", "json": wallets }]}))
                }
                "get_wallet_balance" => {
                    let wallet_name: String = from_value(args["wallet_name"].clone()).unwrap_or_default();
                    let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
                    
                    match storage.get_decrypted_private_key(&wallet_name, &master_password) {
                        Ok(pk) => {
                            let wallet_info = wallet::import_wallet(&pk).unwrap();
                            match client.get_balance(&chain_id, &wallet_info.address).await {
                                Ok(balance) => Ok(json!({ "content": [{ "type": "text", "text": format!("Balance for {}: {} {}", wallet_name, balance.amount, balance.denom) }]})),
                                Err(e) => Err(format!("Failed to get balance: {}", e)),
                            }
                        },
                        Err(e) => Err(format!("Failed to get wallet: {}", e)),
                    }
                }
                "transfer_from_wallet" => {
                    let wallet_name: String = from_value(args["wallet_name"].clone()).unwrap_or_default();
                    let to_address: String = from_value(args["to_address"].clone()).unwrap_or_default();
                    let amount: String = from_value(args["amount"].clone()).unwrap_or_default();
                    let chain_id: String = from_value(args["chain_id"].clone()).unwrap_or_default();
                    
                    match storage.get_decrypted_private_key(&wallet_name, &master_password) {
                        Ok(private_key) => {
                            let transfer_req = SeiTransferRequest {
                                to_address,
                                amount,
                                private_key,
                                gas_limit: from_value(args["gas_limit"].clone()).ok(),
                                gas_price: from_value(args["gas_price"].clone()).ok(),
                            };
                            match client.transfer_sei(&chain_id, &transfer_req).await {
                                Ok(resp) => Ok(json!({ "content": [{ "type": "text", "text": format!("Transaction sent: {}", resp.tx_hash) }]})),
                                Err(e) => Err(format!("Transfer failed: {}", e)),
                            }
                        },
                        Err(e) => Err(format!("Failed to get wallet: {}", e)),
                    }
                }
                "remove_wallet" => {
                    let wallet_name: String = from_value(args["wallet_name"].clone()).unwrap_or_default();
                    if storage.remove_wallet(&wallet_name) {
                        Ok(json!({ "content": [{ "type": "text", "text": format!("Wallet '{}' removed.", wallet_name) }]}) )
                    } else {
                        Err(format!("Wallet '{}' not found.", wallet_name))
                    }
                }
                _ => unreachable!(),
            };

            match result {
                Ok(content) => {
                    if let Err(e) = storage.save_to_file(&storage_path) {
                        return Response::error(req.id.clone(), -32000, format!("Failed to save wallet storage: {}", e));
                    }
                    Response::success(req.id.clone(), content)
                },
                Err(e) => Response::error(req.id.clone(), -32000, e),
            }
        }
        _ => Response::error(req.id.clone(), -32601, format!("Tool '{}' not found", tool_name)),
    }
}
