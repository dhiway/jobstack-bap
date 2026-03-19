use crate::db::{
    job::{fetch_job_by_job_id, JobLookup},
    job_applications::{get_job_applications, store_job_applications, NewJobApplication},
    profiles::{get_or_sync_profile, ProfileLookup},
};
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::payload_generator::build_beckn_payload;
use crate::utils::{external_apis::call_google_geocode, http_client::post_json};
use crate::{
    models::job_apply::{JobApplicationsQuery, JobApplyRequest, JobApplyV2Request},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::oneshot::channel;
use tokio::time::{timeout, Duration};
use tracing::info;
use uuid::Uuid;

use std::sync::Arc;
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn handle_job_apply(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<JobApplyRequest>,
) -> Result<impl IntoResponse, Response> {
    match process_job_apply(&app_state, &req).await {
        Ok(res) => Ok((StatusCode::OK, Json(res))),
        Err((status, err)) => Err((status, Json(err)).into_response()),
    }
}
pub async fn process_job_apply(
    app_state: &Arc<AppState>,
    req: &JobApplyRequest,
) -> Result<serde_json::Value, (StatusCode, serde_json::Value)> {
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
                    json!({"error": "Database error"}),
                ));
            }
        };

    if let Some(application) = existing.into_iter().next() {
        return Ok(json!({
            "message": "User has already applied for this job",
            "application": application
        }));
    }

    let _on_init = match call_and_wait_for_action(app_state, req, "init").await {
        Ok(data) => data,
        Err((status, err)) => return Err((status, json!(err.0))),
    };

    let on_confirm = match call_and_wait_for_action(app_state, req, "confirm").await {
        Ok(data) => data,
        Err((status, err)) => return Err((status, json!(err.0))),
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

    if let Err(e) = store_job_applications(&app_state.db_pool, new_application).await {
        tracing::error!("❌ Failed to store job application: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "Failed to store job application"}),
        ));
    }

    Ok(on_confirm)
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

    if let Err(e) = post_json(&adapter_url, payload, None).await {
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
    State(app_state): State<Arc<AppState>>,
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

pub async fn handle_job_apply_v2(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<JobApplyV2Request>,
) -> Result<impl IntoResponse, Response> {
    let profile = match get_or_sync_profile(&app_state, &req.profile_id).await {
        Ok(p) => p,
        Err((status, err)) => {
            return Err((status, Json(err)).into_response());
        }
    };
    let job = match fetch_job_by_job_id(&app_state.db_pool, &req.job_id).await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("❌ Failed to fetch job: {:?}", e);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Job not found"})),
            )
                .into_response());
        }
    };
    let payload = build_job_apply_payload(&app_state, &job, &profile).await;

    let job_apply_req: JobApplyRequest = match serde_json::from_value(payload) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("❌ Failed to parse JobApplyRequest: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Invalid payload structure"})),
            )
                .into_response());
        }
    };

    let result = match process_job_apply(&app_state, &job_apply_req).await {
        Ok(res) => res,
        Err((status, err)) => {
            return Err((status, Json(err)).into_response());
        }
    };
    Ok((StatusCode::OK, Json(result)))
}

pub async fn build_job_apply_payload(
    app_state: &Arc<AppState>,
    job: &JobLookup,
    profile: &ProfileLookup,
) -> Value {
    let transaction_id = Uuid::new_v4().to_string();

    let profile_id = &profile.profile_id;
    let metadata = &profile.metadata;

    let who_i_am = metadata.get("whoIAm");

    let name = who_i_am
        .and_then(|w| w.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let age = who_i_am
        .and_then(|w| w.get("age"))
        .and_then(|v| v.as_i64())
        .map(|a| a.to_string())
        .unwrap_or_default();

    let gender = who_i_am
        .and_then(|w| w.get("gender"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let phone = who_i_am
        .and_then(|w| w.get("phone"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let address = who_i_am
        .and_then(|w| w.get("location"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let geo_data = if !address.is_empty() {
        match call_google_geocode(app_state, address).await {
            Ok(data) => Some(data),
            Err(e) => {
                tracing::error!("❌ Geocode failed: {:?}", e);
                None
            }
        }
    } else {
        None
    };
    let geo_location = geo_data
        .as_ref()
        .and_then(|g| g.get("results"))
        .and_then(|r| r.get(0))
        .and_then(|r| r.get("geometry"))
        .and_then(|g| g.get("location"));

    let lat = geo_location
        .and_then(|l| l.get("lat"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let lng = geo_location
        .and_then(|l| l.get("lng"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut city_name = "";
    let mut state_name = "";
    let mut state_code = "";
    let mut country_name = "";
    let mut country_code = "";

    if let Some(components) = geo_data
        .as_ref()
        .and_then(|g| g.get("results"))
        .and_then(|r| r.get(0))
        .and_then(|r| r.get("address_components"))
        .and_then(|v| v.as_array())
    {
        for comp in components {
            let types = comp.get("types").and_then(|t| t.as_array());

            let long_name = comp.get("long_name").and_then(|v| v.as_str()).unwrap_or("");
            let short_name = comp
                .get("short_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(types) = types {
                let has_type = |t: &str| types.iter().any(|x| x.as_str() == Some(t));

                if city_name.is_empty()
                    && (has_type("locality")
                        || has_type("sublocality")
                        || has_type("administrative_area_level_2"))
                {
                    city_name = long_name;
                }
                if state_name.is_empty() && has_type("administrative_area_level_1") {
                    state_name = long_name;
                    state_code = short_name;
                }
                if country_name.is_empty() && has_type("country") {
                    country_name = long_name;
                    country_code = short_name;
                }
            }
        }
    }

    let formatted_address = geo_data
        .as_ref()
        .and_then(|g| g.get("results"))
        .and_then(|r| r.get(0))
        .and_then(|r| r.get("formatted_address"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    json!({
        "context": {
            "bpp_id": job.bpp_id.clone().unwrap_or_default(),
            "bpp_uri": job.bpp_uri.clone().unwrap_or_default(),
            "transaction_id": transaction_id
        },
        "message": {
            "order": {
                "provider": {
                    "id": job.provider_id.clone().unwrap_or_default()
                },
                "items": [
                    {
                        "id": job.job_id,
                        "fulfillment_ids": [profile_id]
                    }
                ],
                "fulfillments": [
                    {
                        "id": profile_id,
                        "customer": {
                            "person": {
                                "id": profile_id,
                                "name": name,
                                "age": age,
                                "gender": gender,
                                "metadata": metadata
                            },
                            "contact": {
                                "phone": phone,
                                "email": ""
                            },
                            "location": {
                                "gps": {
                                    "lat": lat,
                                    "lng": lng
                                },
                                "address": formatted_address,
                                "city": {
                                    "name": city_name,
                                    "code": ""
                                },
                                "state": {
                                    "name": state_name,
                                    "code": state_code
                                },
                                "country": {
                                    "name": country_name,
                                    "code": country_code
                                }
                            }
                        }
                    }
                ]
            }
        }
    })
}
