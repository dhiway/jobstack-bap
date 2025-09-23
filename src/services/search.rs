use crate::models::search::SearchRequestV2;
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::{
    models::search::SearchRequest,
    services::payload_generator::build_beckn_payload,
    state::AppState,
    utils::{hash::generate_query_hash, http_client::post_json, search::matches_query_dynamic},
};
use axum::{extract::State, http::StatusCode, Json};
use indexmap::IndexMap;
use redis::AsyncCommands;
use serde_json::{json, Value as JsonValue};
use std::collections::HashSet;
use std::time::Instant;
use tracing::{error, event, info, Level};
use uuid::Uuid;

pub async fn handle_search(
    State(app_state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let start = Instant::now();
    let message_id = format!("msg-{}", Uuid::new_v4());
    let txn_id = format!("txn-{}", Uuid::new_v4());

    let query_hash = generate_query_hash(&req.message);

    let pattern = format!("search:{}:*", query_hash);
    info!("Looking for Redis keys with pattern: {}", pattern);

    // --- Get cached search results ---
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

    // --- Cache txn_id -> query_hash for on_search mapping ---
    {
        let mut conn = app_state.redis_conn.lock().await;
        let txn_key = format!("txn_to_query:{}", txn_id);
        conn.set_ex::<_, _, ()>(&txn_key, &query_hash, app_state.config.cache.txn_ttl_secs)
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

    // --- Throttle BAP calls (dynamic skip time) ---
    let should_call_bap = {
        let mut conn = app_state.redis_conn.lock().await;
        let last_call_key = format!("last_call:{}", query_hash);

        match conn.exists::<_, bool>(&last_call_key).await {
            Ok(exists) if exists => {
                let secs = app_state.config.cache.throttle_secs;
                if secs % 60 == 0 {
                    info!(
                        ": Skipping BAP call (already called within last {} min)",
                        secs / 60
                    );
                } else {
                    info!(
                        ": Skipping BAP call (already called within last {} secs)",
                        secs
                    );
                }
                false
            }
            _ => {
                let _: () = conn
                    .set_ex(&last_call_key, "1", app_state.config.cache.throttle_secs)
                    .await
                    .unwrap_or_default();
                true
            }
        }
    };

    if should_call_bap {
        info!(
            ": Sending search request to BAP adapter at: {}",
            adapter_url
        );
        tokio::spawn(async move {
            if let Err(e) = post_json(&adapter_url, payload).await {
                error!(":x: Failed to send search to BAP adapter: {}", e);
            }
        });
    }

    let elapsed = start.elapsed();
    event!(
        target: "perf",
        Level::INFO,
        transaction_id = %txn_id,
        message_id = %message_id,
        endpoint = "/api/v1/search",
        duration_ms = %elapsed.as_millis(),
        "API timing(search)"
    );

    // --- Return cached results if available ---
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
) -> Json<AckResponse> {
    if txn_id.starts_with("cron-") {
        return handle_cron_on_search(app_state, payload, txn_id).await;
    }

    let mut conn = app_state.redis_conn.lock().await;
    let txn_key = format!("txn_to_query:{}", txn_id);

    match conn.get::<_, String>(&txn_key).await {
        Ok(query_hash) => match &payload.context.bpp_id {
            Some(bpp_id) => {
                let redis_key = format!("search:{}:{}", query_hash, bpp_id);
                match serde_json::to_string(payload) {
                    Ok(data) => {
                        if let Err(e) = conn
                            .set_ex::<_, _, ()>(
                                &redis_key,
                                data,
                                app_state.config.cache.result_ttl_secs,
                            )
                            .await
                        {
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

pub async fn handle_cron_on_search(
    app_state: &AppState,
    payload: &WebhookPayload,
    txn_id: &str,
) -> Json<AckResponse> {
    info!(target: "cron", "📦 Handling cron on_search for txn_id={}", txn_id);

    let mut conn = app_state.redis_conn.lock().await;

    if let Some(bpp_id) = &payload.context.bpp_id {
        let redis_key = format!("cron_jobs:{}:{}", txn_id, bpp_id);

        // Try to get existing stored data from Redis
        let mut store_data: serde_json::Value =
            match conn.get::<_, Option<String>>(&redis_key).await {
                Ok(Some(existing)) => serde_json::from_str(&existing)
                    .unwrap_or_else(|_| serde_json::to_value(payload).unwrap()),
                _ => serde_json::to_value(payload).unwrap(),
            };

        // Append new providers to existing ones
        if let Some(new_providers) = payload
            .message
            .get("catalog")
            .and_then(|c| c.get("providers"))
            .and_then(|p| p.as_array())
        {
            store_data
                .pointer_mut("/message/catalog/providers")
                .and_then(|existing_providers| existing_providers.as_array_mut())
                .map(|arr| arr.extend(new_providers.clone()));
        }

        // Get pagination info from stored data
        let (current_page, limit, total_count) = {
            let pagination = store_data
                .pointer("/message/pagination")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let page = pagination.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
            let limit = pagination
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(30);
            let total_count = pagination
                .get("totalCount")
                .and_then(|v| {
                    if let Some(n) = v.as_i64() {
                        Some(n)
                    } else if let Some(s) = v.as_str() {
                        s.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            (page, limit, total_count)
        };
        info!(
            target: "cron",
            "📄 Pagination status for BPP {}: current_page = {} limit = {} total_count = {}",
            bpp_id,
            current_page,
            limit,
            total_count
        );

        // Store back to Redis with TTL
        let ttl_secs = app_state.config.cache.result_ttl_secs;
        if let Err(e) = conn
            .set_ex::<_, String, ()>(&redis_key, store_data.to_string(), ttl_secs)
            .await
        {
            error!(target: "cron", "❌ Failed to store cron payload for BPP {}: {:?}", bpp_id, e);
        } else {
            info!(target: "cron", "✅ Stored cron payload for BPP {} at {}", bpp_id, redis_key);
        }

        // Handle pagination: request next page if needed
        if current_page * limit < total_count {
            let next_page = current_page + 1;
            info!(
                target: "cron",
                "🔄 More pages to fetch: current_page = {} total_count = {} → requesting next_page = {}",
                current_page,
                total_count,
                next_page
            );

            // Build intent for next page
            let mut intent = payload
                .message
                .get("intent")
                .cloned()
                .unwrap_or_else(|| json!({}));

            intent["item"] = json!({
                "tags": [
                    {
                        "descriptor": { "code": "status", "name": "Status" },
                        "list": [
                            {
                                "descriptor": { "code": "status", "name": "Status" },
                                "value": "open"
                            }
                        ]
                    }
                ]
            });
            // Build final message
            let message = json!({
                "intent": intent,
                "pagination": {
                    "page": next_page,
                    "limit": limit
                },
                "options": {
                    "brief": false
                }
            });

            // Update Redis with next_page prevent duplicate calls
            store_data.pointer_mut("/message/pagination").map(|p| {
                p["page"] = json!(next_page);
            });

            if let Err(e) = conn
                .set_ex::<_, String, ()>(&redis_key, store_data.to_string(), ttl_secs)
                .await
            {
                error!(target: "cron", "❌ Failed to update next_page in Redis: {:?}", e);
            }

            let message_id = format!("msg-{}", Uuid::new_v4());
            let next_payload = build_beckn_payload(
                &app_state.config,
                txn_id,
                &message_id,
                &message,
                "search",
                None,
                None,
            );

            let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);
            if let Err(e) = post_json(&adapter_url, next_payload).await {
                error!(
                    target: "cron",
                    "❌ Failed to request next_page = {} (txn_id={}): {}",
                    next_page,
                    txn_id,
                    e
                );
            } else {
                info!(
                    target: "cron",
                    "📨 Successfully requested next_page = {} for txn_id={}",
                    next_page,
                    txn_id
                );
            }
        } else {
            info!(target: "cron", "✅ All pages fetched for txn_id={}", txn_id);
            info!(target: "cron", "╔════════════════════════════════════════════╗");
            info!(target: "cron", "║   ✅ Finished fetch jobs cron.             ║");
            info!(target: "cron", "╚════════════════════════════════════════════╝");
        }
    } else {
        info!(target: "cron", "⚠️ No bpp_id found in cron payload, skipping storage");
    }

    Json(AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    })
}

pub async fn handle_search_v2(
    State(app_state): State<AppState>,
    Json(req): Json<SearchRequestV2>,
) -> Result<Json<JsonValue>, (StatusCode, Json<JsonValue>)> {
    let mut conn = app_state.redis_conn.lock().await;

    // 👉 Get latest txn_id
    let latest_key = "cron_txn:latest";
    let txn_id: String = match conn.get(latest_key).await {
        Ok(Some(val)) => val,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "No latest txn_id found" })),
            ));
        }
    };

    // Fetch all BPP results for this txn_id
    let pattern = format!("cron_jobs:{}:*", txn_id);
    let keys: Vec<String> = conn.keys(&pattern).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Redis fetch failed: {:?}", e) })),
        )
    })?;

    let page = req.page.unwrap_or(1) as usize;
    let limit = req.limit.unwrap_or(10) as usize;

    let mut seen_ids = HashSet::new();
    let mut flat_items = vec![];

    let provider_filter = req.provider.as_ref().map(|s| s.to_lowercase());
    let role_filters: Vec<String> = req
        .role
        .as_ref()
        .map(|r| r.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();
    let query_filter = req.query.as_ref().map(|s| s.to_lowercase());

    for key in keys {
        if let Ok(Some(payload_str)) = conn.get::<_, Option<String>>(&key).await {
            if let Ok(payload_json) = serde_json::from_str::<JsonValue>(&payload_str) {
                if let Some(providers) = payload_json
                    .pointer("/message/catalog/providers")
                    .and_then(|p| p.as_array())
                {
                    for provider in providers {
                        let provider_name = provider
                            .get("descriptor")
                            .and_then(|d| d.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_lowercase();

                        // provider filter
                        if let Some(ref pf) = provider_filter {
                            if !provider_name.contains(pf) {
                                continue;
                            }
                        }

                        if let Some(items) = provider.get("items").and_then(|i| i.as_array()) {
                            for item in items {
                                let role_name = item
                                    .get("descriptor")
                                    .and_then(|d| d.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();

                                let item_roles: Vec<&str> =
                                    role_name.split(',').map(|s| s.trim()).collect();

                                let mut match_item = true;

                                // role filter
                                if !role_filters.is_empty() {
                                    if !role_filters
                                        .iter()
                                        .any(|rf| item_roles.iter().any(|r| r.contains(rf)))
                                    {
                                        match_item = false;
                                    }
                                }

                                // query filter
                                if let Some(ref qf) = query_filter {
                                    if !matches_query_dynamic(&provider_name, item, qf) {
                                        match_item = false;
                                    }
                                }

                                if match_item {
                                    let id_key = item
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| {
                                            serde_json::to_string(item).unwrap_or_default()
                                        });

                                    if seen_ids.insert(id_key) {
                                        flat_items.push((
                                            payload_json["context"].clone(),
                                            provider.clone(),
                                            item.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let total_count = flat_items.len();

    let start = (page - 1) * limit;
    let paginated_items = flat_items
        .into_iter()
        .skip(start)
        .take(limit)
        .collect::<Vec<_>>();

    //  Group back into payload → providers → items
    let mut results_map: IndexMap<JsonValue, IndexMap<String, (JsonValue, Vec<JsonValue>)>> =
        IndexMap::new();

    for (context, provider, item) in paginated_items {
        let provider_descriptor = provider["descriptor"].clone();
        let provider_id = provider.get("id").cloned().unwrap_or(json!(null));
        let provider_fulfillments = provider.get("fulfillments").cloned().unwrap_or(json!([]));
        let provider_locations = provider.get("locations").cloned().unwrap_or(json!([]));

        let key = serde_json::to_string(&provider_descriptor).unwrap_or_default();

        results_map
            .entry(context.clone())
            .or_default()
            .entry(key)
            .and_modify(|(_, items)| items.push(item.clone()))
            .or_insert_with(|| {
                (
                    json!({
                        "descriptor": provider_descriptor,
                        "id": provider_id,
                        "fulfillments": provider_fulfillments,
                        "locations": provider_locations,
                    }),
                    vec![item.clone()],
                )
            });
    }

    let mut results = vec![];

    for (context, providers_map) in results_map {
        let mut payload = json!({
            "context": context,
            "message": {
                "catalog": {
                    "providers": []
                }
            }
        });

        let providers_arr = providers_map
            .into_iter()
            .map(|(_, (mut provider_obj, items))| {
                provider_obj["items"] = json!(items);
                provider_obj
            })
            .collect::<Vec<_>>();

        payload["message"]["catalog"]["providers"] = json!(providers_arr);
        results.push(payload);
    }

    let response = json!({
        "pagination": {
            "page": page,
            "limit": limit,
            "totalCount": total_count
        },
        "results": results
    });

    Ok(Json(response))
}
