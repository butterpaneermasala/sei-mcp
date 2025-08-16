// src/mcp/handler.rs

use crate::blockchain::models::ChainType;
use crate::{
    blockchain::{
        models::WalletResponse,
        services::{transactions, wallet},
    },
    mcp::{
        protocol::{error_codes, Request, Response},
        wallet_storage,
    },
    utils, AppState,
};
use ethers_core::abi::{encode, Token};
use ethers_core::types::{Address, Bytes, TransactionRequest, U256};
use ethers_core::utils::keccak256;
use ethers_signers::{LocalWallet, Signer};
use reqwest::Client;
use serde_json::{json, Value};
use std::str::FromStr;
use tracing::{error, info};

// Normalize common chain_id aliases users might pass via MCP
pub fn normalize_chain_id(input: &str) -> String {
    // Normalize case and separators first
    let mut s = input.trim().to_lowercase();
    // Replace common separators with '-'
    s = s.replace([' ', '_'], "-");
    // Collapse multiple dashes
    while s.contains("--") {
        s = s.replace("--", "-");
    }

    // Common aliases for EVM networks
    // Accept: sei-testnet, sei-evm-testnet, sei evm testnet, sei_testnet, etc.
    if s == "sei-testnet"
        || s == "sei-evm-testnet"
        || s == "sei-evm-test"
        || s == "sei-evm-t"
        || s == "sei-evm"
    {
        return "sei-evm-testnet".to_string();
    }
    if s == "sei-mainnet" || s == "sei-evm-mainnet" || s == "sei-evm-main" || s == "sei-main" {
        return "sei-evm-mainnet".to_string();
    }

    // Native aliases
    if s == "atlantic-2"
        || s == "sei-native-testnet"
        || s == "sei-native"
        || s == "sei-testnet-native"
    {
        return "atlantic-2".to_string();
    }
    if s == "pacific-1" || s == "sei-native-mainnet" || s == "sei-mainnet-native" {
        return "pacific-1".to_string();
    }

    s
}

// Use the get_required_arg from utils module

// Heuristic: infer EVM chain from natural language in args if chain_id is absent.
// Looks for words like "mainnet" or "testnet" in common text-bearing fields.
fn infer_evm_chain_from_args(args: &Value) -> Option<String> {
    // common fields where NL may be present
    let candidates = [
        "query",
        "text",
        "prompt",
        "instruction",
        "message",
        "description",
    ];
    let mut blob = String::new();
    for key in candidates.iter() {
        if let Some(s) = args.get(*key).and_then(|v| v.as_str()) {
            blob.push_str(" ");
            blob.push_str(s);
        }
    }
    if blob.is_empty() {
        return None;
    }
    let b = blob.to_lowercase();
    if b.contains("mainnet") {
        return Some("sei-evm-mainnet".to_string());
    }
    if b.contains("testnet") {
        return Some("sei-evm-testnet".to_string());
    }
    None
}

// Helper: produce a result Value that always contains a text content array
// and preserves structured data for JSON-friendly clients.
fn make_texty_result(text: String, payload: Value) -> Value {
    let content = json!([{ "type": "text", "text": text }]);
    match payload {
        Value::Object(mut map) => {
            // Do not overwrite if caller already set content
            if !map.contains_key("content") {
                map.insert("content".into(), content);
            }
            Value::Object(map)
        }
        other => json!({
            "data": other,
            "content": content
        }),
    }
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
        // Convenience aliases to support direct method calls from CLI
        // They are rewritten into tools/call internally to reuse the same logic
        "get_balance" | "request_faucet" | "transfer_evm" | "transfer_sei" | "transfer_nft_evm"
        | "search_events" | "get_contract" | "get_contract_code" | "get_contract_transactions"
        | "redirect_to_seidocs" | "get_chain_info" | "get_transaction_info" | "get_transaction_history" | "get_nft_metadata" => {
            let name = req.method.clone();
            let wrapped = Request {
                jsonrpc: req.jsonrpc.clone(),
                id: req.id.clone(),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": name,
                    "arguments": req.params.clone().unwrap_or_else(|| json!({}))
                })),
            };
            handle_tool_call(wrapped, state).await
        }
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
        None => {
            return Response::error(
                req.id,
                error_codes::INVALID_PARAMS,
                "Missing 'params' object".into(),
            )
        }
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => {
            return Response::error(
                req.id,
                error_codes::INVALID_PARAMS,
                "Missing 'name' field in params".into(),
            )
        }
    };

    let empty_args = json!({});
    let args = params.get("arguments").unwrap_or(&empty_args);
    let req_id = &req.id;

    // FIX: All tool logic is now wrapped in an async block for clean error handling
    // and receives the shared application state.
    match tool_name {
        "redirect_to_seidocs" => {
            // Return a simple payload with the docs URL and a text content for MCP clients
            let url = crate::blockchain::services::docs::get_sei_docs_url();
            // Best-effort: on Linux, try opening the default browser via xdg-open
            #[cfg(target_os = "linux")]
            {
                match std::process::Command::new("xdg-open").arg(url).spawn() {
                    Ok(_) => {
                        // Include a link content item for clients that support it
                        let payload = json!({
                            "url": url,
                            "opened": true,
                            "content": [
                                { "type": "text", "text": "Opened Sei documentation in your browser" },
                                { "type": "link", "text": "Sei Docs", "url": url }
                            ]
                        });
                        return Response::success(req_id.clone(), payload);
                    }
                    Err(_e) => {
                        // Ignore and continue to return the link-only payload below
                    }
                }
            }
            let payload = json!({ "url": url });
            let summary = "Open Sei documentation".to_string();
            // Some MCP clients don't support a dedicated 'link' content item; keep it text-only
            Response::success(
                req_id.clone(),
                json!({
                    "url": url,
                    "content": [ { "type": "text", "text": format!("{}: {}", summary, url) } ]
                })
            )
        }
        ,
        // --- SeiStream read-only tools ---
        "get_chain_info" => {
            let res: Result<Response, Response> = (async {
                let client = Client::new();
                let v = crate::blockchain::services::seistream::get_chain_info(&client)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                // Provide both human-friendly text content and raw JSON for clients to parse
                let latest = v.get("latestBlock").and_then(|b| b.get("height")).and_then(|h| h.as_u64());
                let network = v.get("network").and_then(|n| n.as_str()).unwrap_or("unknown");
                let summary = if let Some(h) = latest { format!("Chain info — {} (height {})", network, h) } else { format!("Chain info — {}", network) };
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        // flatten key fields for clients that render top-level data
                        "network": v.get("network"),
                        "latestBlock": v.get("latestBlock"),
                        "validators": v.get("validators"),
                        "window": v.get("window"),
                        // full payload for programmatic consumers
                        "data": v,
                        // human summary
                        "content": [ { "type": "text", "text": summary } ]
                    }),
                ))
            })
            .await;
            match res { Ok(r) => r, Err(e) => e }
        }
        "get_transaction_info" => {
            let res: Result<Response, Response> = (async {
                let hash = utils::get_required_arg::<String>(args, "hash", req_id)?;
                let client = Client::new();
                let v = crate::blockchain::services::seistream::get_transaction_info(&client, &hash)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                let from = v.get("from").and_then(|s| s.as_str()).unwrap_or("");
                let to = v.get("to").and_then(|s| s.as_str()).unwrap_or("");
                let summary = if !status.is_empty() {
                    format!("Tx {} — {} ({} -> {})", &hash, status, from, to)
                } else {
                    format!("Tx {}", &hash)
                };
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        // expose common tx fields at top-level when present
                        "hash": v.get("hash").cloned().unwrap_or_else(|| json!(hash)),
                        "from": v.get("from"),
                        "to": v.get("to"),
                        "status": v.get("status"),
                        "blockNumber": v.get("blockNumber"),
                        // full payload
                        "data": v,
                        // human summary
                        "content": [ { "type": "text", "text": summary } ]
                    })
                ))
            })
            .await;
            match res { Ok(r) => r, Err(e) => e }
        }
        "get_transaction_history" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                let page = args.get("page").and_then(|v| v.as_u64());
                let client = Client::new();
                let v = crate::blockchain::services::seistream::get_transaction_history(&client, &address, page)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                let count = v.get("items").and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(0);
                let summary = match page {
                    Some(p) => format!("History for {} — {} item(s) on page {}", &address, count, p),
                    None => format!("History for {} — {} item(s)", &address, count),
                };
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        "data": v,
                        "content": [ { "type": "text", "text": summary } ]
                    })
                ))
            })
            .await;
            match res { Ok(r) => r, Err(e) => e }
        }
        "get_nft_metadata" => {
            let res: Result<Response, Response> = (async {
                // ERC-721 items for a contract
                let contract = utils::get_required_arg::<String>(args, "contract_address", req_id)?;
                let page = args.get("page").and_then(|v| v.as_u64());
                let client = Client::new();
                let v = crate::blockchain::services::seistream::get_nft_metadata_erc721_items(&client, &contract, page)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                let count = v.get("items").and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(0);
                let summary = match page {
                    Some(p) => format!("ERC-721 items for {} — {} item(s) on page {}", &contract, count, p),
                    None => format!("ERC-721 items for {} — {} item(s)", &contract, count),
                };
                // Optionally include the first item inline as text preview for Claude UX
                let preview = v.get("items").and_then(|i| i.as_array()).and_then(|a| a.get(0)).cloned();
                let mut content = vec![ json!({ "type": "text", "text": summary }) ];
                if let Some(first) = preview {
                    if let Ok(pretty) = serde_json::to_string_pretty(&first) {
                        content.push(json!({ "type": "text", "text": format!("Preview (first item):\n{}", pretty) }));
                    }
                }
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        // top-level helpful fields for clients
                        "contract_address": contract,
                        "page": page,
                        "count": count,
                        // first item preview also as structured field
                        "preview": v.get("items").and_then(|i| i.as_array()).and_then(|a| a.get(0)).cloned(),
                        // full payload
                        "data": v,
                        // human summary and preview text
                        "content": content
                    })
                ))
            })
            .await;
            match res { Ok(r) => r, Err(e) => e }
        }
        "discord_post_message" => {
            let res: Result<Response, Response> = (async {
                let message = utils::get_required_arg::<String>(args, "message", req_id)?;
                let username = args.get("username").and_then(|v| v.as_str());
                let res = crate::api::discord::post_discord_message(&state, &message, username)
                    .await
                    .map_err(|e| Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()))?;
                let summary = if let Some(u) = username {
                    format!("Posted to Discord as '{}'", u)
                } else {
                    "Posted to Discord".to_string()
                };
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, res),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }
        "get_balance" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                let mut chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                chain_id = normalize_chain_id(&chain_id);
                let rpc_url = match state.config.chain_rpc_urls.get(&chain_id) {
                    Some(u) => u,
                    None => {
                        let keys: Vec<String> =
                            state.config.chain_rpc_urls.keys().cloned().collect();
                        return Err(Response::error(
                            req_id.clone(),
                            error_codes::INVALID_PARAMS,
                            format!(
                                "RPC URL not configured for chain_id '{}'. Available: {}",
                                chain_id,
                                keys.join(", ")
                            ),
                        ));
                    }
                };
                let chain_type = ChainType::from_chain_id(&chain_id);
                let client = Client::new();
                let is_native = matches!(chain_type, ChainType::Native);
                let balance = crate::blockchain::services::balance::get_balance(
                    &client, rpc_url, &address, is_native,
                )
                .await
                .map_err(|e| {
                    Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                })?;
                let debug_info = json!({
                    "chain_id_normalized": chain_id,
                    "rpc_url": rpc_url,
                    "chain_type": if is_native { "native" } else { "evm" }
                });
                let balance_text = match serde_json::to_string(&balance) {
                    Ok(s) => format!("Balance: {}", s),
                    Err(_) => "Balance fetched".to_string(),
                };
                // Return plain JSON so MCP clients can parse result directly
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        // Plain fields for Windsurf and generic JSON-RPC clients
                        "balance": balance,
                        "debug": debug_info,
                        "message": balance_text,
                        // Text content for clients that expect a content array
                        "content": [
                            { "type": "text", "text": balance_text }
                        ]
                    }),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "create_wallet" => match state.sei_client.create_wallet().await {
            Ok(wallet) => {
                let summary = format!("Created wallet {}", wallet.address);
                Response::success(req_id.clone(), make_texty_result(summary, json!(wallet)))
            }
            Err(e) => Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string()),
        },

        "import_wallet" => {
            let res: Result<Response, Response> = (async {
                let key = utils::get_required_arg::<String>(args, "key", req_id)?;
                let wallet = state.sei_client.import_wallet(&key).await.map_err(|e| {
                    Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                })?;
                let summary = format!("Imported wallet {}", wallet.address);
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, json!(wallet)),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "request_faucet" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                let mut chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                chain_id = normalize_chain_id(&chain_id);
                let rpc_url = match state.config.chain_rpc_urls.get(&chain_id) {
                    Some(u) => u,
                    None => {
                        let keys: Vec<String> =
                            state.config.chain_rpc_urls.keys().cloned().collect();
                        return Err(Response::error(
                            req_id.clone(),
                            error_codes::INVALID_PARAMS,
                            format!(
                                "RPC URL not configured for chain_id '{}'. Available: {}",
                                chain_id,
                                keys.join(", ")
                            ),
                        ));
                    }
                };
                let tx_hash = crate::blockchain::services::faucet::send_faucet_tokens(
                    &state.config,
                    &address,
                    &state.nonce_manager,
                    rpc_url,
                    &chain_id,
                )
                .await
                .map_err(|e| {
                    Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                })?;
                let payload = json!({ "transaction_hash": tx_hash });
                let summary = format!("Faucet sent tokens: tx {}", tx_hash);
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, payload),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Event tools ---
        "search_events" => {
            let res: Result<Response, Response> = (async {
                let chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                match ChainType::from_chain_id(&chain_id) {
                    ChainType::Evm => {
                        let rpc_url =
                            state.config.chain_rpc_urls.get(&chain_id).ok_or_else(|| {
                                Response::error(
                                    req_id.clone(),
                                    error_codes::INVALID_PARAMS,
                                    format!("RPC URL not configured for chain_id '{}'", chain_id),
                                )
                            })?;
                        let address = args
                            .get("contract_address")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                Response::error(
                                    req_id.clone(),
                                    error_codes::INVALID_PARAMS,
                                    "Missing 'contract_address'".into(),
                                )
                            })?;
                        let from_block = args.get("from_block").and_then(|v| v.as_str());
                        let to_block = args.get("to_block").and_then(|v| v.as_str());
                        let topic0 = args.get("topic0").and_then(|v| v.as_str());

                        // Helper to normalize block tags: accept hex tags (latest/earliest/pending) or decimal block numbers.
                        fn normalize_block_tag(tag: &str) -> String {
                            let t = tag.trim();
                            if t == "latest"
                                || t == "earliest"
                                || t == "pending"
                                || t.starts_with("0x")
                            {
                                return t.to_string();
                            }
                            // Try parse as decimal number
                            if let Ok(n) = u64::from_str_radix(t, 10) {
                                return format!("0x{:x}", n);
                            }
                            t.to_string()
                        }

                        let mut filter = serde_json::json!({ "address": address });
                        if let Some(fb) = from_block {
                            filter["fromBlock"] =
                                serde_json::Value::String(normalize_block_tag(fb));
                        }
                        if let Some(tb) = to_block {
                            filter["toBlock"] = serde_json::Value::String(normalize_block_tag(tb));
                        }
                        if let Some(t0) = topic0 {
                            filter["topics"] = serde_json::json!([t0]);
                        }

                        let payload = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "eth_getLogs",
                            "params": [filter],
                            "id": 1
                        });
                        let client = Client::new();
                        let resp: serde_json::Value = client
                            .post(rpc_url)
                            .json(&payload)
                            .send()
                            .await
                            .map_err(|e| {
                                Response::error(
                                    req_id.clone(),
                                    error_codes::INTERNAL_ERROR,
                                    format!("RPC error: {}", e),
                                )
                            })?
                            .json()
                            .await
                            .map_err(|e| {
                                Response::error(
                                    req_id.clone(),
                                    error_codes::INTERNAL_ERROR,
                                    format!("Invalid RPC JSON: {}", e),
                                )
                            })?;
                        if let Some(err) = resp.get("error") {
                            return Err(Response::error(
                                req_id.clone(),
                                error_codes::INTERNAL_ERROR,
                                format!("RPC error: {}", err),
                            ));
                        }
                        // Wrap logs with a summary text
                        let logs = resp["result"].clone();
                        let count = logs.as_array().map(|a| a.len()).unwrap_or(0);
                        let payload = json!({ "logs": logs });
                        let summary = format!("Found {} log(s)", count);
                        Ok(Response::success(
                            req_id.clone(),
                            make_texty_result(summary, payload),
                        ))
                    }
                    ChainType::Native => Err(Response::error(
                        req_id.clone(),
                        error_codes::INTERNAL_ERROR,
                        "Native event search not implemented yet".into(),
                    )),
                }
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Transfers ---
        // EVM value transfer using a provided private key
        "transfer_evm" => {
            let res: Result<Response, Response> = (async {
                let private_key = utils::get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = utils::get_required_arg::<String>(args, "to_address", req_id)?;
                let amount_wei = utils::get_required_arg::<String>(args, "amount_wei", req_id)?;

                let to = Address::from_str(&to_address).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'to_address'".into(),
                    )
                })?;
                let value = U256::from_dec_str(&amount_wei).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'amount_wei'".into(),
                    )
                })?;

                let mut tx_request = TransactionRequest::new().to(to).value(value);
                if let Some(g) = args.get("gas_limit").and_then(|v| v.as_str()) {
                    tx_request =
                        tx_request.gas(U256::from_dec_str(g).unwrap_or_else(|_| U256::from(0)));
                }
                if let Some(gp) = args.get("gas_price").and_then(|v| v.as_str()) {
                    tx_request = tx_request
                        .gas_price(U256::from_dec_str(gp).unwrap_or_else(|_| U256::from(0)));
                }

                let response = state
                    .sei_client
                    .send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                let summary = match serde_json::to_string(&response) {
                    Ok(s) => format!("EVM tx sent: {}", s),
                    Err(_) => "EVM tx sent".to_string(),
                };
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, json!(response)),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // Native SEI bank transfer using a provided Cosmos private key (0x-hex secp256k1)
        "transfer_sei" => {
            let res: Result<Response, Response> = (async {
                let private_key = utils::get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = utils::get_required_arg::<String>(args, "to_address", req_id)?;
                let amount_usei = utils::get_required_arg::<String>(args, "amount_usei", req_id)?;

                let amount = amount_usei.parse::<u64>().map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'amount_usei'".into(),
                    )
                })?;
                let rpc_url = state.config.chain_rpc_urls.get(&chain_id).ok_or_else(|| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        format!("RPC URL not configured for chain_id '{}'", chain_id),
                    )
                })?;

                let tx_hash = transactions::send_native_transaction_signed(
                    &state.config,
                    rpc_url,
                    &private_key,
                    &to_address,
                    amount,
                )
                .await
                .map_err(|e| {
                    Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                })?;
                let payload = json!({ "transaction_hash": tx_hash });
                let summary = format!("SEI bank tx: {}", tx_hash);
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, payload),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // EVM ERC-721 transfer
        "transfer_nft_evm" => {
            let res: Result<Response, Response> = (async {
                let private_key = utils::get_required_arg::<String>(args, "private_key", req_id)?;
                let chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                let contract_address =
                    utils::get_required_arg::<String>(args, "contract_address", req_id)?;
                let to_address = utils::get_required_arg::<String>(args, "to_address", req_id)?;
                let token_id = utils::get_required_arg::<String>(args, "token_id", req_id)?;

                let wallet = LocalWallet::from_str(&private_key).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'private_key'".into(),
                    )
                })?;
                let from_addr = wallet.address();
                let to = Address::from_str(&to_address).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'to_address'".into(),
                    )
                })?;
                let contract = Address::from_str(&contract_address).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'contract_address'".into(),
                    )
                })?;
                let token_u256 = U256::from_dec_str(&token_id).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'token_id'".into(),
                    )
                })?;

                // Encode safeTransferFrom(address,address,uint256)
                let selector =
                    &keccak256("safeTransferFrom(address,address,uint256)".as_bytes())[0..4];
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

                let mut tx_request = TransactionRequest::new()
                    .to(contract)
                    .data(data_bytes)
                    .value(U256::zero());
                if let Some(g) = args.get("gas_limit").and_then(|v| v.as_str()) {
                    tx_request =
                        tx_request.gas(U256::from_dec_str(g).unwrap_or_else(|_| U256::from(0)));
                }
                if let Some(gp) = args.get("gas_price").and_then(|v| v.as_str()) {
                    tx_request = tx_request
                        .gas_price(U256::from_dec_str(gp).unwrap_or_else(|_| U256::from(0)));
                }

                let response = state
                    .sei_client
                    .send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                Ok(Response::success(req_id.clone(), json!(response)))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        // --- Secure Wallet Storage Tools ---
        "register_wallet" => {
            let res: Result<Response, Response> = (async {
                let wallet_name = utils::get_required_arg::<String>(args, "wallet_name", req_id)?;
                let private_key = utils::get_required_arg::<String>(args, "private_key", req_id)?;
                let master_password =
                    utils::get_required_arg::<String>(args, "master_password", req_id)?;

                let wallet_info: WalletResponse =
                    wallet::import_wallet(&private_key).map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INVALID_PARAMS, e.to_string())
                    })?;

                let mut storage = state.wallet_storage.lock().await;
                if !storage.verify_master_password(&master_password) {
                    return Err(Response::error(
                        req_id.clone(),
                        error_codes::INTERNAL_ERROR,
                        "Invalid master password".into(),
                    ));
                }

                storage
                    .add_wallet(
                        wallet_name.clone(),
                        &private_key,
                        wallet_info.address,
                        &master_password,
                    )
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;

                wallet_storage::save_wallet_storage(&state.wallet_storage_path, &storage).map_err(
                    |e| {
                        error!("Failed to save wallet storage: {}", e);
                        Response::error(
                            req_id.clone(),
                            error_codes::INTERNAL_ERROR,
                            "Failed to save wallet to disk".into(),
                        )
                    },
                )?;

                let payload = json!({ "status": "success", "wallet_name": wallet_name });
                let summary = format!("Registered wallet {}", wallet_name);
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, payload),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "list_wallets" => {
            let res: Result<Response, Response> = (async {
                let master_password =
                    utils::get_required_arg::<String>(args, "master_password", req_id)?;
                let storage = state.wallet_storage.lock().await;
                if !storage.verify_master_password(&master_password) {
                    return Err(Response::error(
                        req_id.clone(),
                        error_codes::INTERNAL_ERROR,
                        "Invalid master password".into(),
                    ));
                }
                let wallets = storage.list_wallets();
                let count = wallets.len();
                let payload = json!({ "wallets": wallets });
                let summary = format!("{} wallet(s)", count);
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, payload),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }

        "transfer_from_wallet" => {
            let res: Result<Response, Response> = (async {
                let wallet_name = utils::get_required_arg::<String>(args, "wallet_name", req_id)?;
                let chain_id = utils::get_required_arg::<String>(args, "chain_id", req_id)?;
                let to_address = utils::get_required_arg::<String>(args, "to_address", req_id)?;
                let amount = utils::get_required_arg::<String>(args, "amount", req_id)?;
                let master_password =
                    utils::get_required_arg::<String>(args, "master_password", req_id)?;

                let private_key = {
                    // Scoped lock
                    let storage = state.wallet_storage.lock().await;
                    storage
                        .get_decrypted_private_key(&wallet_name, &master_password)
                        .map_err(|e| {
                            Response::error(
                                req_id.clone(),
                                error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            )
                        })?
                };

                let to = Address::from_str(&to_address).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'to_address'".into(),
                    )
                })?;
                let value = U256::from_dec_str(&amount).map_err(|_| {
                    Response::error(
                        req_id.clone(),
                        error_codes::INVALID_PARAMS,
                        "Invalid 'amount'".into(),
                    )
                })?;

                let tx_request = TransactionRequest::new().to(to).value(value);

                let response = state
                    .sei_client
                    .send_transaction(&chain_id, &private_key, tx_request, &state.nonce_manager)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                let summary = match serde_json::to_string(&response) {
                    Ok(s) => format!("Transfer sent: {}", s),
                    Err(_) => "Transfer sent".to_string(),
                };
                Ok(Response::success(
                    req_id.clone(),
                    make_texty_result(summary, json!(response)),
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }
        "get_contract" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                // Prefer explicit chain_id, else infer from NL, default to testnet
                let mut chain = args
                    .get("chain_id")
                    .and_then(|v| v.as_str())
                    .map(normalize_chain_id);
                if chain.is_none() {
                    chain = infer_evm_chain_from_args(args);
                }
                let chain_id = chain.unwrap_or_else(|| "sei-evm-testnet".to_string());
                let contract = state
                    .sei_client
                    .get_contract(&chain_id, &address)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                let summary = format!("Contract {} on {}", address, chain_id);
                let pretty = serde_json::to_string_pretty(&contract).unwrap_or_else(|_| contract.to_string());
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        "content": [
                            { "type": "text", "text": format!("{}\n\n{}", summary, pretty) }
                        ]
                    })
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }
        "get_contract_code" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                let mut chain = args
                    .get("chain_id")
                    .and_then(|v| v.as_str())
                    .map(normalize_chain_id);
                if chain.is_none() {
                    chain = infer_evm_chain_from_args(args);
                }
                let chain_id = chain.unwrap_or_else(|| "sei-evm-testnet".to_string());
                let code = state
                    .sei_client
                    .get_contract_code(&chain_id, &address)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                let summary = format!("Contract code for {} on {}", address, chain_id);
                let pretty = serde_json::to_string_pretty(&code).unwrap_or_else(|_| code.to_string());
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        "content": [
                            { "type": "text", "text": format!("{}\n\n{}", summary, pretty) }
                        ]
                    })
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }
        "get_contract_transactions" => {
            let res: Result<Response, Response> = (async {
                let address = utils::get_required_arg::<String>(args, "address", req_id)?;
                let mut chain = args
                    .get("chain_id")
                    .and_then(|v| v.as_str())
                    .map(normalize_chain_id);
                if chain.is_none() {
                    chain = infer_evm_chain_from_args(args);
                }
                let chain_id = chain.unwrap_or_else(|| "sei-evm-testnet".to_string());
                let txs = state
                    .sei_client
                    .get_contract_transactions(&chain_id, &address)
                    .await
                    .map_err(|e| {
                        Response::error(req_id.clone(), error_codes::INTERNAL_ERROR, e.to_string())
                    })?;
                let count = txs.get("items").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                let summary = format!("{} tx(s) for {} on {}", count, address, chain_id);
                let pretty = serde_json::to_string_pretty(&txs).unwrap_or_else(|_| txs.to_string());
                Ok(Response::success(
                    req_id.clone(),
                    json!({
                        "content": [
                            { "type": "text", "text": format!("{}\n\n{}", summary, pretty) }
                        ]
                    })
                ))
            })
            .await;
            res.unwrap_or_else(|err_resp| err_resp)
        }
        _ => Response::error(
            req.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Tool not found: {}", tool_name),
        ),
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
            "name": "redirect_to_seidocs",
            "description": "Return the Sei documentation URL (https://docs.sei.io/).",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
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
        },
         {
            "name": "get_contract",
            "description": "Get general details for a smart contract.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": {"type": "string", "description": "The address of the smart contract."},
                    "chain_id": {"type": "string", "description": "Optional chain id (e.g., 'sei-evm-mainnet' or 'sei-evm-testnet')."}
                },
                "required": ["address"]
            }
        },
        {
            "name": "get_contract_code",
            "description": "Get the code of a smart contract.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": {"type": "string", "description": "The address of the smart contract."}
                },
                "required": ["address"]
            }
        },
        {
            "name": "discord_post_message",
            "description": "Post a message to Discord via webhook or bot token (configured in server env).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Text to post. Code or natural language supported."},
                    "username": {"type": "string", "description": "Optional display username (webhook mode)."}
                },
                "required": ["message"],
                "additionalProperties": false
            }
        },
        { 
            "name": "get_contract_transactions",
            "description": "Get the transactions of a smart contract.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": {"type": "string", "description": "The address of the smart contract."}
                },
                "required": ["address"]
            }
        },
        {
            "name": "get_chain_info",
            "description": "Get general chain info from SeiStream (network, latest block, validators, etc).",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        },
        {
            "name": "get_transaction_info",
            "description": "Get transaction info by EVM hash from SeiStream.",
            "inputSchema": {
                "type": "object",
                "properties": {"hash": {"type": "string"}},
                "required": ["hash"],
                "additionalProperties": false
            }
        },
        {
            "name": "get_transaction_history",
            "description": "Get transaction history for an EVM address from SeiStream.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": {"type": "string"},
                    "page": {"type": "number", "description": "Optional page number"}
                },
                "required": ["address"],
                "additionalProperties": false
            }
        },
        {
            "name": "get_nft_metadata",
            "description": "Get ERC-721 NFT metadata items for a contract from SeiStream.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract_address": {"type": "string"},
                    "page": {"type": "number", "description": "Optional page number"}
                },
                "required": ["contract_address"],
                "additionalProperties": false
            }
        },
    ]);
    Response::success(req.id.clone(), json!({ "tools": tools }))
}
