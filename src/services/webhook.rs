use crate::models::core::Context;
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::{
    job_apply::{handle_on_confirm, handle_on_init},
    profiles::handle_search_profiles,
    search::handle_on_search,
    select::handle_on_select,
    status::handle_on_status,
};
use crate::state::AppState;
use crate::workers::processor::spawn_processing_task;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info};
pub async fn webhook_handler(
    Path(action): Path<String>,
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    let txn_id = payload.context.transaction_id.clone();
    let msg_id = payload.context.message_id.clone();
    info!("webhook called: action = {}, txn_id = {}", action, txn_id);
    println!("payload: {:?}", payload.context);

    let response = match action.as_str() {
        "on_search" => handle_on_search(&app_state, &payload, &txn_id)
            .await
            .into_response(),
        "on_select" => handle_on_select(&app_state, &payload, &txn_id, &msg_id)
            .await
            .into_response(),
        "on_init" => handle_on_init(&app_state, &payload, &txn_id, &msg_id)
            .await
            .into_response(),
        "on_confirm" => handle_on_confirm(&app_state, &payload, &txn_id, &msg_id)
            .await
            .into_response(),
        "on_status" => handle_on_status(&app_state, &payload, &txn_id, &msg_id)
            .await
            .into_response(),
        _ => Json(AckResponse {
            message: AckStatus {
                ack: Ack { status: "ACK" },
            },
        })
        .into_response(),
    };

    response
}

pub async fn webhook_handler_profiles(
    Path(action): Path<String>,
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    info!(
        target: "webhook",
        "🟢 [ Adapter → BPP] Request received | txn_id: {}, msg_id: {}, action: {}, timestamp: {}",
        payload.context.transaction_id,
        payload.context.message_id,
        payload.context.action,
        payload.context.timestamp
    );
    debug!(target: "webhook", "🔎 Message payload: {:?}", payload.message);
    if action.starts_with("on_") {
        info!(
            "Skipping processing since action starts with 'on_': {:?}",
            action
        );
        let ack = AckResponse {
            message: AckStatus {
                ack: Ack { status: "ACK" },
            },
        };
        return Json(ack);
    }

    spawn_processing_task(payload.context, payload.message, action, app_state);

    let ack = AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    };

    Json(ack)
}

pub async fn generate_response(
    action: &str,
    context: Context,
    message: Value,
    state: &AppState,
) -> anyhow::Result<Value> {
    match action {
        "search" => handle_search_profiles(context, message, state).await,
        _ => Err(anyhow::anyhow!("Unknown action: {}", action)),
    }
}
