use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::{
    job_apply::{handle_on_confirm, handle_on_init},
    search::handle_on_search,
    select::handle_on_select,
    status::handle_on_status,
};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use tracing::info;

pub async fn webhook_handler(
    Path(action): Path<String>,
    State(app_state): State<AppState>,
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
