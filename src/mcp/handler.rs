// src/mcp/handler.rs

use crate::{
    AppState,
    mcp::{
        protocol::{error_codes, Request, Response},
        wallet_storage,
    },
    blockchain::{models::WalletResponse, services::{wallet, transactions}},
};
use ethers_core::types::{Address, TransactionRequest, U256, Bytes};
use ethers_core::utils::keccak256;
use ethers_core::abi::{encode, Token};
use ethers_signers::{LocalWallet, Signer};
use crate::blockchain::models::ChainType;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{from_value, json, Value};
use std::str::FromStr;
use tracing::{error, info};

// Normalize common chain_id aliases users might pass via MCP
pub fn normalize_chain_id(input: &str) -> String {
    let mut s = input.trim().to_string();
    if s == "sei-testnet" { s = "sei-evm-testnet".to_string(); }
    if s == "sei-mainnet" { s = "sei-evm-mainnet".to_string(); }
    s
}

// FIX: A helper function for safe argument parsing.
// It returns a proper JSON-RPC error response if a required argument is missing or invalid.
fn get_required_arg<T: DeserializeOwned>(
    args: &Value,
    name: &str,
    req_id: &Value,
) -> Result<T, Response> {
    from_value(args[name].clone()).map_err(|_| {
        Response::error(
            req_id.clone(),
            error_codes::INVALID_PARAMS,
            format!("Missing or invalid required argument: '{}'", name),
        )
    })
}

/// This is the main dispatcher for all incoming MCP requests.
pub async fn handle_mcp_request(req: Request, state: AppState) -> Option<Response> {
    info!("Handling MCP request for method: {}", req.method);

    if req.is_notification() {
        return None;
    }

    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "tools/list" => handle_tools_list(&req),
        "tools/call" => handle_tool_call(req, state).await,
        _ => Response::error(
            req.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", req.method),
        ),
    };

    Some(response)
}

/// Handles a 'tools/call' request by dispatching it to the correct tool logic.
async fn handle_tool_call(req: Request, state: AppState) -> Response {
    let params = match req.params.as_ref() {
        Some(p) => p,
        None => return Response::error(req.id, error_codes::INVALID_PARAMS, "Missing 'params' object".into()),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => return Response::error(req.id, error_codes::INVALID_PARAMS, "Missing 'name' field in params".into()),
    };

    let empty_args = json!({});
    let args = params.get("arguments").unwrap_or(&empty_args);
    let req_id = &req.id;

    // FIX: All tool logic is now wrapped in an async block for clean error handling
    // and receives the shared application state.
    match tool_name {
        "get_balance" => {
            let res: Result<Response, Response> = (async {
                let address = get_required_arg::<String>(args, "address", req_id)?;
                let mut chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                chain_id = normalize_chain_id(&chain_id);
                let rpc_url = match state.config.chain_rpc_urls.get(&chain_id) {
                    Some(u) => u,
                    None => {
                        let keys: Vec<String> = state.config.chain_rpc_urls.keys().cloned().collect();
                        return Err(Response::error(
                            req_id.clone(),
                            error_codes::INVALID_PARAMS,
                            format!("RPC URL not configured for chain_id '{}'. Available: {}", chain_id, keys.join(", ")),
                        ));
                    }
                };
                let chain_type = ChainType::from_chain_id(&chain_id);
                let client = Client::new();
                let is_native = matches!(chain_type, ChainType::Native);
                let balance = crate::blockchain::services::balance::get_balance(&client, rpc_url, &address, is_native).await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                Ok(Response::success(req_id.clone(), json!({"balance": balance})))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "create_wallet" => match state.sei_client.create_wallet().await {
            Ok(wallet) => Response::success(req_id.clone(), json!({ "content": [{ "type": "json", "json": wallet }]})),
            Err(e) => Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()),
        },

        "import_wallet" => {
            let res: Result<Response, Response> = (async {
                let key = get_required_arg::<String>(args, "mnemonic_or_private_key", req_id)?;
                let wallet = state.sei_client.import_wallet(&key).await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                Ok(Response::success(req_id.clone(), json!({ "content": [{ "type": "json", "json": wallet }] })))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "request_faucet" => {
            let res: Result<Response, Response> = (async {
                let address = get_required_arg::<String>(args, "address", req_id)?;
                let mut chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                chain_id = normalize_chain_id(&chain_id);
                let rpc_url = match state.config.chain_rpc_urls.get(&chain_id) {
                    Some(u) => u,
                    None => {
                        let keys: Vec<String> = state.config.chain_rpc_urls.keys().cloned().collect();
                        return Err(Response::error(
                            req_id.clone(),
                            error_codes::INVALID_PARAMS,
                            format!("RPC URL not configured for chain_id '{}'. Available: {}", chain_id, keys.join(", ")),
                        ));
                    }
                };
                let tx_hash = crate::blockchain::services::faucet::send_faucet_tokens(&state.config, &address, &state.nonce_manager, rpc_url, &chain_id).await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                Ok(Response::success(req_id.clone(), json!({ "transaction_hash": tx_hash })))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Event tools ---
        "search_events" => {
            let res: Result<Response, Response> = (async {
                let chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                match ChainType::from_chain_id(&chain_id) {
                    ChainType::Evm => {
                        let rpc_url = state.config.chain_rpc_urls.get(&chain_id)
                            .ok_or_else(|| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, format!("RPC URL not configured for chain_id '{}'", chain_id)))?;
                        let address = args.get("contract_address").and_then(|v| v.as_str()).ok_or_else(|| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Missing 'contract_address'".into()))?;
                        let from_block = args.get("from_block").and_then(|v| v.as_str());
                        let to_block = args.get("to_block").and_then(|v| v.as_str());
                        let topic0 = args.get("topic0").and_then(|v| v.as_str());

                        let mut filter = serde_json::json!({ "address": address });
                        if let Some(fb) = from_block { filter["fromBlock"] = serde_json::Value::String(fb.to_string()); }
                        if let Some(tb) = to_block { filter["toBlock"] = serde_json::Value::String(tb.to_string()); }
                        if let Some(t0) = topic0 { filter["topics"] = serde_json::json!([t0]); }

                        let payload = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "eth_getLogs",
                            "params": [filter],
                            "id": 1
                        });
                        let client = Client::new();
                        let resp: serde_json::Value = client.post(rpc_url).json(&payload).send().await
                            .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, format!("RPC error: {}", e)))?
                            .json().await
                            .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, format!("Invalid RPC JSON: {}", e)))?;
                        if let Some(err) = resp.get("error") {
                            return Err(Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, format!("RPC error: {}", err)));
                        }
                        Ok(Response::success(req_id.clone(), resp["result"].clone()))
                    }
                    ChainType::Native => {
                        Err(Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, "Native event search not implemented yet".into()))
                    }
                }
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Transfers ---
        // EVM value transfer using a provided private key
        "transfer_evm" => {
            let res: Result<Response, Response> = (async {
                let private_key = get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = get_required_arg::<String>(args, "to_address", req_id)?;
                let amount_wei = get_required_arg::<String>(args, "amount_wei", req_id)?;

                let to = Address::from_str(&to_address)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'to_address'".into()))?;
                let value = U256::from_dec_str(&amount_wei)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'amount_wei'".into()))?;

                let mut tx_request = TransactionRequest::new().to(to).value(value);
                if let Some(g) = args.get("gas_limit").and_then(|v| v.as_str()) {
                    tx_request = tx_request.gas(U256::from_dec_str(g).unwrap_or_else(|_| U256::from(0)));
                }
                if let Some(gp) = args.get("gas_price").and_then(|v| v.as_str()) {
                    tx_request = tx_request.gas_price(U256::from_dec_str(gp).unwrap_or_else(|_| U256::from(0)));
                }

                let response = state.sei_client
                    .send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                Ok(Response::success(req_id.clone(), json!(response)))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // Native SEI bank transfer using a provided Cosmos private key (0x-hex secp256k1)
        "transfer_sei" => {
            let res: Result<Response, Response> = (async {
                let private_key = get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = get_required_arg::<String>(args, "to_address", req_id)?;
                let amount_usei = get_required_arg::<String>(args, "amount_usei", req_id)?;

                let amount = amount_usei.parse::<u64>()
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'amount_usei'".into()))?;
                let rpc_url = state.config.chain_rpc_urls.get(&chain_id)
                    .ok_or_else(|| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, format!("RPC URL not configured for chain_id '{}'", chain_id)))?;

                let tx_hash = transactions::send_native_transaction_signed(
                    &state.config,
                    rpc_url,
                    &private_key,
                    &to_address,
                    amount,
                ).await.map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;

                Ok(Response::success(req_id.clone(), json!({ "transaction_hash": tx_hash })))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // EVM ERC-721 transfer
        "transfer_nft_evm" => {
            let res: Result<Response, Response> = (async {
                let private_key = get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                let contract_address = get_required_arg::<String>(args, "contract_address", req_id)?;
                let to_address = get_required_arg::<String>(args, "to_address", req_id)?;
                let token_id = get_required_arg::<String>(args, "token_id", req_id)?;

                let wallet = LocalWallet::from_str(&private_key)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'private_key'".into()))?;
                let from_addr = wallet.address();
                let to = Address::from_str(&to_address)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'to_address'".into()))?;
                let contract = Address::from_str(&contract_address)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'contract_address'".into()))?;
                let token_u256 = U256::from_dec_str(&token_id)
                    .map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'token_id'".into()))?;

                // Encode safeTransferFrom(address,address,uint256)
                let selector = &keccak256("safeTransferFrom(address,address,uint256)".as_bytes())[0..4];
                let data_bytes = {
                    let mut encoded = selector.to_vec();
                    let tokens = vec![
                        Token::Address(from_addr.into()),
                        Token::Address(to.into()),
                        Token::Uint(token_u256.into()),
                    ];
                    let mut tail = encode(&tokens);
                    encoded.append(&mut tail);
                    Bytes::from(encoded)
                };

                let mut tx_request = TransactionRequest::new().to(contract).data(data_bytes).value(U256::zero());
                if let Some(g) = args.get("gas_limit").and_then(|v| v.as_str()) {
                    tx_request = tx_request.gas(U256::from_dec_str(g).unwrap_or_else(|_| U256::from(0)));
                }
                if let Some(gp) = args.get("gas_price").and_then(|v| v.as_str()) {
                    tx_request = tx_request.gas_price(U256::from_dec_str(gp).unwrap_or_else(|_| U256::from(0)));
                }

                let response = state.sei_client
                    .send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                Ok(Response::success(req_id.clone(), json!(response)))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Secure Wallet Storage Tools ---

        "register_wallet" => {
            let res: Result<Response, Response> = (async {
                let wallet_name = get_required_arg::<String>(args, "wallet_name", req_id)?;
                let private_key = get_required_arg::<String>(args, "private_key", req_id)?;
                let master_password = get_required_arg::<String>(args, "master_password", req_id)?;
                
                let wallet_info: WalletResponse = wallet::import_wallet(&private_key)
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, e.to_string()))?;

                let mut storage = state.wallet_storage.lock().await;
                if !storage.verify_master_password(&master_password) {
                    return Err(Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, "Invalid master password".into()));
                }
                
                storage.add_wallet(wallet_name.clone(), &private_key, wallet_info.address, &master_password)
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;

                wallet_storage::save_wallet_storage(&state.wallet_storage_path, &storage)
                        .map_err(|e| {
                            error!("Failed to save wallet storage: {}", e);
                            Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, "Failed to save wallet to disk".into())
                        })?;
                
                Ok(Response::success(req_id.clone(), json!({ "status": "success", "wallet_name": wallet_name })))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "list_wallets" => {
            let res: Result<Response, Response> = (async {
                let master_password = get_required_arg::<String>(args, "master_password", req_id)?;
                let storage = state.wallet_storage.lock().await;
                if !storage.verify_master_password(&master_password) {
                    return Err(Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, "Invalid master password".into()));
                }
                let wallets = storage.list_wallets();
                Ok(Response::success(req_id.clone(), json!({ "wallets": wallets })))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "transfer_from_wallet" => {
            let res: Result<Response, Response> = (async {
                let wallet_name = get_required_arg::<String>(args, "wallet_name", req_id)?;
                let chain_id = get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = get_required_arg::<String>(args, "to_address", req_id)?;
                let amount = get_required_arg::<String>(args, "amount", req_id)?;
                let master_password = get_required_arg::<String>(args, "master_password", req_id)?;
                
                let private_key = { // Scoped lock
                    let storage = state.wallet_storage.lock().await;
                    storage.get_decrypted_private_key(&wallet_name, &master_password)
                        .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?
                };
                
                let to = Address::from_str(&to_address).map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'to_address'".into()))?;
                let value = U256::from_dec_str(&amount).map_err(|_| Response::error(req_id.clone(), error_codes::INVALID_PARAMS, "Invalid 'amount'".into()))?;

                let tx_request = TransactionRequest::new().to(to).value(value);

                let response = state.sei_client.send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager).await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                
                Ok(Response::success(req_id.clone(), json!(response)))
            }).await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        _ => Response::error(req.id, error_codes::METHOD_NOT_FOUND, format!("Tool not found: {}", tool_name)),
    }
}

/// Handles the 'initialize' request.
fn handle_initialize(req: &Request) -> Response {
    let server_info = json!({
        "name": "sei-mcp-server-rs",
        "version": "0.2.0-fixed"
    });
    let capabilities = json!({ "tools": { "listChanged": false } });
    let instructions =
        "Sei EVM blockchain MCP server for secure wallet operations, balance queries, and transaction management.";

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
// FIX: The tool list is now updated, secure, and functional.
fn handle_tools_list(req: &Request) -> Response {
    let tools = json!([
        {
            "name": "get_balance",
            "description": "Get the EVM balance of an address on a specific Sei chain.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "The blockchain chain ID (e.g., 'sei-testnet')"},
                    "address": {"type": "string", "description": "The 0x... EVM wallet address to check."}
                },
                "required": ["chain_id", "address"]
            }
        },
        {
            "name": "create_wallet",
            "description": "Create a new EVM wallet. Returns address, private key, and mnemonic.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "import_wallet",
            "description": "Import an EVM wallet from a mnemonic phrase or private key.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mnemonic_or_private_key": {"type": "string", "description": "The mnemonic phrase or private key to import."}
                },
                "required": ["mnemonic_or_private_key"]
            }
        },
        {
            "name": "search_events",
            "description": "Search EVM logs via eth_getLogs. For native events, not yet implemented.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string"},
                    "contract_address": {"type": "string"},
                    "topic0": {"type": "string", "description": "Keccak topic0 (event signature hash)"},
                    "from_block": {"type": "string", "description": "hex block tag like '0x1' or 'earliest'"},
                    "to_block": {"type": "string", "description": "hex block tag like 'latest'"}
                },
                "required": ["chain_id", "contract_address"],
                "additionalProperties": false
            }
        },
        {
            "name": "request_faucet",
            "description": "Request testnet tokens from the faucet for an EVM address.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chain_id": {"type": "string", "description": "Target chain id configured in CHAIN_RPC_URLS."},
                    "address": {"type": "string", "description": "The EVM (0x...) address to receive tokens."}
                },
                "required": ["chain_id", "address"],
                "additionalProperties": false
            }
        },
        {
            "name": "register_wallet",
            "description": "Encrypt and securely store a private key under a wallet name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "A unique name for the wallet (e.g., 'my-primary-wallet')."},
                    "private_key": {"type": "string", "description": "The private key to encrypt and store."},
                    "master_password": {"type": "string", "description": "The master password to encrypt the wallet. This password will be required for any future actions with this wallet."}
                },
                "required": ["wallet_name", "private_key", "master_password"]
            }
        },
        {
            "name": "list_wallets",
            "description": "List the names of all wallets currently stored in the secure storage.",
            "inputSchema": {
                "type": "object",
                "properties": {
                     "master_password": {"type": "string", "description": "The master password for the wallet storage."}
                },
                "required": ["master_password"]
            }
        },
        {
            "name": "transfer_from_wallet",
            "description": "Transfer tokens from a securely stored wallet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wallet_name": {"type": "string", "description": "The name of the stored wallet to transfer from."},
                    "chain_id": {"type": "string", "description": "The blockchain chain ID (e.g., 'sei-testnet')."},
                    "to_address": {"type": "string", "description": "The recipient's 0x... EVM address."},
                    "amount": {"type": "string", "description": "The amount to transfer in the smallest unit (e.g., usei)."},
                    "master_password": {"type": "string", "description": "The master password to unlock the wallet for this transaction."}
                },
                "required": ["wallet_name", "chain_id", "to_address", "amount", "master_password"]
            }
        },
        {
            "name": "transfer_evm",
            "description": "Send an EVM value transfer using a provided private key.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "private_key": {"type": "string"},
                    "chain_id": {"type": "string"},
                    "to_address": {"type": "string"},
                    "amount_wei": {"type": "string"},
                    "gas_limit": {"type": "string"},
                    "gas_price": {"type": "string"}
                },
                "required": ["private_key", "chain_id", "to_address", "amount_wei"],
                "additionalProperties": false
            }
        },
        {
            "name": "transfer_sei",
            "description": "Send a native SEI (Cosmos) bank transfer using a provided private key.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "private_key": {"type": "string", "description": "0x-hex Cosmos secp256k1 private key"},
                    "chain_id": {"type": "string"},
                    "to_address": {"type": "string", "description": "Bech32 address (sei...)"},
                    "amount_usei": {"type": "string"}
                },
                "required": ["private_key", "chain_id", "to_address", "amount_usei"],
                "additionalProperties": false
            }
        },
        {
            "name": "transfer_nft_evm",
            "description": "Transfer an ERC-721 token (placeholder).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "private_key": {"type": "string"},
                    "chain_id": {"type": "string"},
                    "contract_address": {"type": "string"},
                    "to_address": {"type": "string"},
                    "token_id": {"type": "string"}
                },
                "required": ["private_key", "chain_id", "contract_address", "to_address", "token_id"],
                "additionalProperties": false
            }
        }
    ]);
    Response::success(req.id.clone(), json!({ "tools": tools }))
}