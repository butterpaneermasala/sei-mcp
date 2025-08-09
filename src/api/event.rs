use crate::blockchain::client::SeiClient;
use crate::config::AppConfig;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SearchQuery {
    pub event_type: Option<String>,
    pub attribute_key: Option<String>,
    pub attribute_value: Option<String>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

#[derive(Deserialize, Debug)]
pub struct ContractEventsQuery {
    pub contract_address: String,
    pub event_type: Option<String>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
    pub page: Option<u32>,
    pub per_page: Option<u8>,
}

/// GET /search-events
/// Searches for past transaction events based on various criteria.
/// Query Parameters:
/// - event_type: e.g., "wasm"
/// - attribute_key: e.g., "action"
/// - attribute_value: e.g., "transfer"
/// - from_block, to_block: Block height range.
/// - page, per_page: Pagination.
pub async fn search_events(
    State(config): State<AppConfig>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    let event_query = crate::blockchain::models::EventQuery {
        contract_address: None,
        event_type: query.event_type.clone(),
        attribute_key: query.attribute_key.clone(),
        attribute_value: query.attribute_value.clone(),
        from_block: query.from_block,
        to_block: query.to_block,
    };

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(30);

    match crate::blockchain::services::event::search_events(&client, event_query, page, per_page)
        .await
    {
        Ok(result) => Ok(Json(serde_json::to_value(result).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Serialization error: {}", e),
            )
        })?)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// GET /get-contract-events
/// Fetches historical events for a specific contract.
/// Query Parameters:
/// - contract_address: The address of the smart contract.
/// - event_type: Optional event type to filter by.
/// - from_block, to_block: Block height range.
/// - page, per_page: Pagination.
pub async fn get_contract_events(
    State(config): State<AppConfig>,
    Query(query): Query<ContractEventsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let client = SeiClient::new(&config.chain_rpc_urls, config.websocket_url.clone());

    let event_query = crate::blockchain::models::EventQuery {
        contract_address: Some(query.contract_address.clone()),
        event_type: query.event_type.clone(),
        attribute_key: None,   // Not used for direct contract event search
        attribute_value: None, // Not used for direct contract event search
        from_block: query.from_block,
        to_block: query.to_block,
    };

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(30);

    match crate::blockchain::services::event::search_events(&client, event_query, page, per_page)
        .await
    {
        Ok(result) => Ok(Json(serde_json::to_value(result).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Serialization error: {}", e),
            )
        })?)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// GET /subscribe-contract-events?contract_address={address}
/// Subscribes to live events from a specific contract via WebSocket.
/// Note: WebSocket support requires additional setup in axum.
pub async fn subscribe_contract_events(
    State(_config): State<AppConfig>,
    Query(query): Query<ContractEventsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // For now, return a message indicating WebSocket support is not yet implemented
    // TODO: Implement proper WebSocket support for axum
    Ok(Json(serde_json::json!({
        "message": "WebSocket subscription not yet implemented for axum",
        "contract_address": query.contract_address
    })))
}
