use crate::services::payload_generator::build_beckn_payload;
use crate::{models::status::StatusRequest, state::AppState, utils::http_client::post_json};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tokio::sync::oneshot::channel;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use tracing::info;

pub async fn handle_status(
    State(app_state): State<AppState>,
    Json(req): Json<StatusRequest>,
) -> Result<impl IntoResponse, Response> {
    let ctx = &req.context;

    let transaction_id = ctx.transaction_id.clone();
    let message_id = format!("msg-{}", Uuid::new_v4());
    let (tx, rx) = channel();
    let unique_key = format!("{}:{}", transaction_id, message_id);
    app_state
        .shared_state
        .pending_searches
        .insert(unique_key, tx);

    let config = app_state.config.clone();
    let adapter_url = format!("{}/status", config.bap.caller_uri);

    let payload = build_beckn_payload(
        &config,
        &transaction_id,
        &message_id,
        &req.message,
        "status",
        Some(&req.context.bpp_id),
        Some(&req.context.bpp_uri),
    );

    if let Err(e) = post_json(&adapter_url, payload, None).await {
        app_state
            .shared_state
            .pending_searches
            .remove(&transaction_id);
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Error calling BAP adapter: {}", e),
            }),
        )
            .into_response());
    }

    match timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(on_select_payload)) => Ok(Json(on_select_payload)),
        Ok(Err(_recv_err)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal channel error receiving on_status".to_string(),
            }),
        )
            .into_response()),
        Err(_elapsed) => {
            app_state
                .shared_state
                .pending_searches
                .remove(&transaction_id);
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                Json(ErrorResponse {
                    error: "Timeout waiting for on_status response".to_string(),
                }),
            )
                .into_response())
        }
    }
}

pub async fn handle_on_status(
    app_state: &AppState,
    payload: &WebhookPayload,
    txn_id: &str,
    msg_id: &str,
) -> impl IntoResponse {
    let unique_key = format!("{}:{}", txn_id, msg_id);
    match app_state.shared_state.pending_searches.remove(&unique_key) {
        Some((_, sender)) => match serde_json::to_value(payload) {
            Ok(json_value) => {
                if let Err(e) = sender.send(json_value) {
                    info!("⚠️ Failed to send on_status payload: {:?}", e);
                } else {
                    info!("✅ Delivered on_status to waiting request");
                }
            }
            Err(e) => {
                info!("❌ Failed to serialize payload: {:?}", e);
            }
        },
        None => {
            info!(
                "⚠️ No pending /status request found for transaction_id = {}",
                txn_id
            );
        }
    }

    Json(AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    })
}
