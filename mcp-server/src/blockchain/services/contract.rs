// src/blockchain/services/contract.rs

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

// Seistream contract API (chain-agnostic base; network inferred by address)
const SEISCAN_API_MAINNET: &str = "https://api.seistream.app/contracts/evm";
const SEISCAN_API_TESTNET: &str = "https://api.seistream.app/contracts/evm";

fn get_seiscan_api_base(chain_id: &str) -> &str {
    // Currently the API host/path does not vary per chain; keep function for future flexibility.
    let _ = chain_id; // suppress unused warning in case of future use
    SEISCAN_API_MAINNET
}

pub async fn get_contract(client: &Client, chain_id: &str, address: &str) -> Result<Value> {
    let base_url = get_seiscan_api_base(chain_id);
    let url = format!("{}/{}", base_url, address);
    let res = client.get(&url).send().await?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&body) {
        Ok(v) => Ok(v),
        Err(_) => {
            // Return a wrapper to avoid decode errors while surfacing raw body
            Ok(serde_json::json!({
                "status": status.as_u16(),
                "raw": body
            }))
        }
    }
}

pub async fn get_contract_code(client: &Client, chain_id: &str, address: &str) -> Result<Value> {
    let base_url = get_seiscan_api_base(chain_id);
    let url = format!("{}/{}/code", base_url, address);
    let res = client.get(&url).send().await?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&body) {
        Ok(v) => Ok(normalize_contract_code(v)),
        Err(_) => Ok(serde_json::json!({ "status": status.as_u16(), "raw": body })),
    }
}

pub async fn get_contract_transactions(
    client: &Client,
    chain_id: &str,
    address: &str,
) -> Result<Value> {
    let base_url = get_seiscan_api_base(chain_id);
    let url = format!("{}/{}/transactions", base_url, address);
    let res = client.get(&url).send().await?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&body) {
        Ok(v) => Ok(v),
        Err(_) => Ok(serde_json::json!({ "status": status.as_u16(), "raw": body })),
    }
}

// Normalize upstream contract code JSON into the strict schema required by clients.
// Target schema:
// {
//   "abi": ["string"],
//   "compilerSettings": [ { ... } ],
//   "externalLibraries": [ { ... } ],
//   "runtimeCode": "string",
//   "creationCode": "string",
//   "sources": [ { "name": "string", "sourceCode": "string" } ]
// }
fn normalize_contract_code(v: Value) -> Value {
    use serde_json::json;

    // abi: coerce any array elements into strings; if object, stringify; else empty
    let abi_arr = v.get("abi").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let abi: Vec<String> = abi_arr
        .into_iter()
        .map(|el| match el {
            Value::String(s) => s,
            other => other.to_string(),
        })
        .collect();

    // compilerSettings: accept object or array; coerce to array of objects
    let compiler_settings = match v.get("compilerSettings") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::Object(_)) => vec![v.get("compilerSettings").unwrap().clone()],
        _ => vec![],
    };

    // externalLibraries: accept array or object; coerce to array
    let external_libraries = match v.get("externalLibraries") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::Object(_)) => vec![v.get("externalLibraries").unwrap().clone()],
        _ => vec![],
    };

    // runtimeCode / creationCode: support camelCase and snake_case fallbacks
    let runtime_code = v
        .get("runtimeCode")
        .or_else(|| v.get("runtime_code"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let creation_code = v
        .get("creationCode")
        .or_else(|| v.get("creation_code"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    // sources: if array, map to desired; if object map name -> {content|source|sourceCode}, convert to array
    let sources = match v.get("sources") {
        Some(Value::Array(arr)) => {
            let mapped: Vec<Value> = arr
                .iter()
                .map(|item| {
                    if let Value::Object(obj) = item {
                        let name = obj.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let sc = obj
                            .get("sourceCode")
                            .or_else(|| obj.get("content"))
                            .or_else(|| obj.get("source"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        json!({ "name": name, "sourceCode": sc })
                    } else {
                        json!({ "name": "", "sourceCode": item.to_string() })
                    }
                })
                .collect();
            mapped
        }
        Some(Value::Object(map)) => {
            let mut out: Vec<Value> = Vec::new();
            for (name, val) in map.iter() {
                let source_code = match val {
                    Value::String(s) => s.clone(),
                    Value::Object(o) => o
                        .get("content")
                        .or_else(|| o.get("source"))
                        .or_else(|| o.get("sourceCode"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    other => other.to_string(),
                };
                out.push(json!({ "name": name, "sourceCode": source_code }));
            }
            out
        }
        Some(Value::String(s)) => vec![json!({ "name": "", "sourceCode": s })],
        _ => vec![],
    };

    json!({
        "abi": abi,
        "compilerSettings": compiler_settings,
        "externalLibraries": external_libraries,
        "runtimeCode": runtime_code,
        "creationCode": creation_code,
        "sources": sources
    })
}

