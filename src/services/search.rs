use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::{
    models::search::SearchRequest,
    services::payload_generator::build_beckn_payload,
    state::AppState,
    utils::{hash::generate_query_hash, http_client::post_json},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use redis::AsyncCommands;
use tracing::{error, info};
use uuid::Uuid;

pub async fn handle_search(
    State(app_state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let message_id = format!("msg-{}", Uuid::new_v4());
    let txn_id = format!("txn-{}", Uuid::new_v4());

    let query_hash = generate_query_hash(&req.message);

    let pattern = format!("search:{}:*", query_hash);
    info!("Looking for Redis keys with pattern: {}", pattern);

    let mut all_keys = {
        let mut conn = app_state.redis_conn.lock().await;

        let mut stream = conn.scan_match::<_, String>(&pattern).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to scan Redis",
                    "details": e.to_string()
                })),
            )
        })?;

        let mut keys = vec![];
        while let Some(k) = stream.next_item().await {
            keys.push(k);
        }
        keys
    };

    all_keys.sort();

    info!("Matched Redis keys: {:?}", all_keys);

    let cached_results = {
        let mut conn = app_state.redis_conn.lock().await;
        let mut results = vec![];

        for key in &all_keys {
            match conn.get::<_, String>(key).await {
                Ok(value) => {
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&value) {
                        results.push(json_value);
                    } else {
                        error!("Failed to parse cached value for key: {}", key);
                    }
                }
                Err(e) => error!("Redis get error for key {}: {}", key, e),
            }
        }

        results
    };

    {
        let mut conn = app_state.redis_conn.lock().await;
        let txn_key = format!("txn_to_query:{}", txn_id);
        conn.set_ex::<_, _, ()>(&txn_key, &query_hash, 300)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "Failed to cache txn_id",
                        "details": e.to_string()
                    })),
                )
            })?;
    }

    let config = app_state.config.clone();
    let payload = build_beckn_payload(
        &config,
        &txn_id,
        &message_id,
        &req.message,
        "search",
        None,
        None,
    );
    let adapter_url = format!("{}/search", config.bap.caller_uri);
    info!("Sending search request to BAP adapter at: {}", adapter_url);

    tokio::spawn(async move {
        if let Err(e) = post_json(&adapter_url, payload).await {
            error!("❌ Failed to send search to BAP adapter: {}", e);
        }
    });

    if !cached_results.is_empty() {
        return Ok(Json(serde_json::json!({
            "results": cached_results
        })));
    }

    Ok(Json(serde_json::json!([])))
}

pub async fn handle_on_search(
    app_state: &AppState,
    payload: &WebhookPayload,
    txn_id: &str,
) -> impl IntoResponse {
    let mut conn = app_state.redis_conn.lock().await;
    let txn_key = format!("txn_to_query:{}", txn_id);

    match conn.get::<_, String>(&txn_key).await {
        Ok(query_hash) => match &payload.context.bpp_id {
            Some(bpp_id) => {
                let redis_key = format!("search:{}:{}", query_hash, bpp_id);
                match serde_json::to_string(payload) {
                    Ok(data) => {
                        if let Err(e) = conn.set_ex::<_, _, ()>(&redis_key, data, 300).await {
                            info!("❌ Failed to store in Redis: {:?}", e);
                        } else {
                            info!("✅ Stored response at key: {}", redis_key);
                        }
                    }
                    Err(e) => {
                        info!("❌ Failed to serialize payload: {:?}", e);
                    }
                }
            }
            None => {
                info!("⚠️ No bpp_id found in payload, skipping Redis cache");
            }
        },
        Err(_) => {
            info!("❌ No query_hash found for txn_id = {}", txn_id);
        }
    }

    Json(AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    })
}
