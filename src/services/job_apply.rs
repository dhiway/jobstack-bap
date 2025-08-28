use crate::db::job_applications::{
    get_job_applications, store_job_applications, NewJobApplication,
};
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::payload_generator::build_beckn_payload;
use crate::utils::http_client::post_json;
use crate::{
    models::job_apply::{JobApplicationsQuery, JobApplyRequest},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;
use tokio::sync::oneshot::channel;
use tokio::time::{timeout, Duration};
use tracing::info;
use uuid::Uuid;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn handle_job_apply(
    State(app_state): State<AppState>,
    Json(req): Json<JobApplyRequest>,
) -> Result<impl IntoResponse, Response> {
    let user_id = req
        .message
        .order
        .fulfillments
        .as_ref()
        .and_then(|fulfillments| fulfillments.get(0))
        .and_then(|f| f.customer.as_ref())
        .map(|c| c.person.id.clone())
        .unwrap_or_else(|| "".to_string());

    let job_id = req
        .message
        .order
        .items
        .get(0)
        .map(|item| item.id.clone())
        .unwrap_or_else(|| "".to_string());

    let existing =
        match get_job_applications(&app_state.db_pool, Some(&user_id), Some(&job_id)).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("DB error: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Database error"})),
                )
                    .into_response());
            }
        };
    if let Some(application) = existing.into_iter().next() {
        let response_body = json!({
            "message": "User has already applied for this job",
            "application": application
        });
        return Ok((StatusCode::OK, axum::Json(response_body)));
    }

    let _on_init = match call_and_wait_for_action(&app_state, &req, "init").await {
        Ok(data) => data,
        Err((status, err)) => return Err((status, err).into_response()),
    };
    let on_confirm = match call_and_wait_for_action(&app_state, &req, "confirm").await {
        Ok(data) => data,
        Err((status, err)) => return Err((status, err).into_response()),
    };

    let transaction_id = on_confirm["context"]["transaction_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let bpp_id = on_confirm["context"]["bpp_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let bpp_uri = on_confirm["context"]["bpp_uri"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let user_id = on_confirm["message"]["order"]["fulfillments"]
        .get(0)
        .and_then(|f| f["customer"]["person"]["id"].as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            on_confirm["message"]["order"]["fulfillments"]
                .get(0)
                .and_then(|f| f["id"].as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let job_id = on_confirm["message"]["order"]["items"]
        .get(0)
        .and_then(|i| i["id"].as_str())
        .unwrap_or_default()
        .to_string();

    let order_id = on_confirm["message"]["order"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let new_application = NewJobApplication {
        user_id,
        job_id,
        order_id,
        transaction_id,
        bpp_id,
        bpp_uri,
        status: Some("APPLIED".to_string()),
        metadata: Some(on_confirm.clone()),
    };
    //  Store in DB
    if let Err(e) = store_job_applications(&app_state.db_pool, new_application).await {
        tracing::error!("❌ Failed to store job application: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to store job application"})),
        )
            .into_response());
    }

    Ok((StatusCode::OK, Json(on_confirm)))
}

async fn call_and_wait_for_action(
    app_state: &AppState,
    req: &JobApplyRequest,
    action: &str,
) -> Result<serde_json::Value, (StatusCode, Json<ErrorResponse>)> {
    let transaction_id = &req.context.transaction_id;
    let message_id = format!("msg-{}", Uuid::new_v4());
    let unique_key = format!("{}:{}", transaction_id, message_id);

    let (tx, rx) = channel();

    app_state
        .shared_state
        .pending_searches
        .insert(unique_key.clone(), tx);

    let config = &app_state.config;
    let adapter_url = format!("{}/{}", config.bap.caller_uri, action);

    let payload = build_beckn_payload(
        config,
        transaction_id,
        &message_id,
        &req.message,
        action,
        Some(&req.context.bpp_id),
        Some(&req.context.bpp_uri),
    );

    if let Err(e) = post_json(&adapter_url, payload).await {
        app_state.shared_state.pending_searches.remove(&unique_key);
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Error calling BAP adapter: {}", e),
            }),
        ));
    }

    match timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(on_action_payload)) => Ok(on_action_payload),
        Ok(Err(_recv)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Internal error receiving on_{} response", action),
            }),
        )),
        Err(_elapsed) => {
            app_state.shared_state.pending_searches.remove(&unique_key);
            Err((
                StatusCode::GATEWAY_TIMEOUT,
                Json(ErrorResponse {
                    error: format!("Timeout waiting for on_{} response", action),
                }),
            ))
        }
    }
}

pub async fn handle_on_init(
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
                    info!("⚠️ Failed to send on_init payload: {:?}", e);
                } else {
                    info!("✅ Delivered on_init to waiting request");
                }
            }
            Err(e) => {
                info!("❌ Failed to serialize payload: {:?}", e);
            }
        },
        None => {
            info!(
                "⚠️ No pending /init request found for transaction_id = {}",
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

pub async fn handle_on_confirm(
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
                    info!("⚠️ Failed to send on_confirm payload: {:?}", e);
                } else {
                    info!("✅ Delivered on_confirm to waiting request");
                }
            }
            Err(e) => {
                info!("❌ Failed to serialize payload: {:?}", e);
            }
        },
        None => {
            info!(
                "⚠️ No pending /confirm request found for transaction_id = {}",
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

pub async fn handle_job_applications(
    State(app_state): State<AppState>,
    Query(params): Query<JobApplicationsQuery>,
) -> impl IntoResponse {
    let user_id = &params.user_id;

    let result = get_job_applications(&app_state.db_pool, Some(user_id), None).await;

    match result {
        Ok(applications) => {
            let apps_json: Vec<_> = applications
                .into_iter()
                .map(|app| {
                    json!({
                        "job_id": app.job_id,
                        "status": app.status,
                        "order_id": app.order_id,
                        "transaction_id": app.transaction_id,
                        "bpp_id": app.bpp_id,
                        "bpp_uri": app.bpp_uri,
                        "metadata": app.metadata,
                    })
                })
                .collect();

            let body = json!({
                "user_id": user_id,
                "applications": apps_json,
            });

            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            tracing::error!("DB error fetching job applications: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
                .into_response()
        }
    }
}
