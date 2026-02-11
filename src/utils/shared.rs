use crate::config::AppConfig;
use crate::models::webhook::{Ack, AckResponse, AckStatus};
use crate::utils::http_client::post_json;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use axum::Json;

pub async fn send_to_bpp_caller(
    action: &str,
    payload: Value,
    config: Arc<AppConfig>,
) -> Result<()> {
    let txn_id = payload
        .get("context")
        .and_then(|ctx| ctx.get("transaction_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_txn");
    let full_action = format!("on_{}", action);

    info!(
        target: "bpp",
         "🟡 [BPP → Adapter] Sending request | action: {}, txn_id: {}",
        full_action,
        txn_id
    );
    info!(target: "bpp", "──────────────────────────────────────────────");

    let bpp_url = &config.bpp.caller_uri;
    let full_url = format!("{}/{}", bpp_url.trim_end_matches('/'), full_action);
    post_json(&full_url, payload, None).await
}

pub fn ack() -> Json<AckResponse> {
    Json(AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    })
}
