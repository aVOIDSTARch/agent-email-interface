use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::mail::{AgentMailTransport, AgentMessage};

use super::{error::ApiError, state::AppState};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

#[derive(Deserialize)]
pub struct SendRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<Value>, ApiError> {
    state.mail.send(&req.to, &req.subject, &req.body).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn fetch_unread(
    State(state): State<AppState>,
) -> Result<Json<Vec<AgentMessage>>, ApiError> {
    let messages = state.mail.fetch_unread().await?;
    Ok(Json(messages))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<AgentMessage>>, ApiError> {
    let messages = state.mail.search(&params.q).await?;
    Ok(Json(messages))
}

pub async fn get_message(
    State(state): State<AppState>,
    Path(uid): Path<u32>,
) -> Result<Json<AgentMessage>, ApiError> {
    state
        .mail
        .get_by_uid(uid)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub async fn mark_read(
    State(state): State<AppState>,
    Path(uid): Path<u32>,
) -> Result<Json<Value>, ApiError> {
    state.mail.mark_read(uid).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_mailboxes(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let mailboxes = state.mail.list_mailboxes().await?;
    Ok(Json(mailboxes))
}
