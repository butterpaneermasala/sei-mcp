// src/api/discord.rs

use axum::{extract::State, http::StatusCode, Json};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;

use crate::AppState;

#[derive(Deserialize)]
pub struct DiscordPostRequest {
    pub message: String,
    pub username: Option<String>,
}

#[derive(Serialize)]
struct WebhookBody<'a> {
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a str>,
}

pub async fn post_discord_message(
    state: &AppState,
    content: &str,
    username: Option<&str>,
) -> anyhow::Result<Value> {
    // Delegate to unified service implementation
    let res = crate::blockchain::services::discord::send_message(state, content, username).await?;
    Ok(res)
}

pub async fn post_discord_handler(
    State(state): State<AppState>,
    Json(req): Json<DiscordPostRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let res = post_discord_message(&state, &req.message, req.username.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(res))
}
