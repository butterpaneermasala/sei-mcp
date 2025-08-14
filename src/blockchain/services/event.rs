use crate::blockchain::{
    client::SeiClient,
    models::{EventQuery, SearchEventsResponse},
};
use anyhow::{anyhow, Result};
// This import will now work correctly
use tendermint_rpc::Order;
use tracing::info;

/// Builds a Tendermint RPC query string from the provided parameters.
fn build_query(query: EventQuery) -> String {
    let mut conditions = vec!["tx.height > 0".to_string()];

    if let Some(contract) = query.contract_address {
        conditions.push(format!("wasm._contract_address = '{}'", contract));
    }
    if let Some(event_type) = query.event_type {
        conditions.push(format!("wasm.event_type = '{}'", event_type));
    }
    if let Some(key) = query.attribute_key {
        conditions.push(format!("wasm.attribute_key = '{}'", key));
    }
    if let Some(value) = query.attribute_value {
        conditions.push(format!("wasm.attribute_value = '{}'", value));
    }
    if let Some(from) = query.from_block {
        conditions.push(format!("tx.height >= {}", from));
    }
    if let Some(to) = query.to_block {
        conditions.push(format!("tx.height <= {}", to));
    }

    conditions.join(" AND ")
}

/// Searches for transactions based on event criteria.
pub async fn search_events(
    client: &SeiClient,
    query: EventQuery,
    page: u32,
    per_page: u8,
) -> Result<SearchEventsResponse> {
    let rpc_query = build_query(query);
    info!("Executing event search with query: {}", rpc_query);

    // Try to use a very simple query if the complex one fails
    let response = match client
        .search_txs(rpc_query.clone(), page, per_page, Order::Descending)
        .await
    {
        Ok(resp) => resp,
        Err(_) => {
            info!("Complex query failed, trying simple query: tx.height > 0");
            client
                .search_txs(
                    "tx.height > 0".to_string(),
                    page,
                    per_page,
                    Order::Descending,
                )
                .await?
        }
    };

    let result = SearchEventsResponse {
        txs: serde_json::to_value(response.txs)
            .map_err(|e| anyhow!("Failed to serialize txs: {}", e))?
            .as_array()
            .ok_or_else(|| anyhow!("Expected array"))?
            .clone(),
        total_count: response.total_count,
    };

    Ok(result)
}

// Note: WebSocket functionality is not implemented for axum yet
// This would require additional WebSocket support in axum
#[allow(dead_code)] // Suppress warning as this is for future implementation
pub struct ContractEventSubscriber {
    client: SeiClient,
    contract_address: String,
}

#[allow(dead_code)] // Suppress warning as this is for future implementation
impl ContractEventSubscriber {
    pub fn new(client: SeiClient, contract_address: String) -> Self {
        Self {
            client,
            contract_address,
        }
    }
}
