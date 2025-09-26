use crate::models::search::SearchRequestV2;
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::empeding::{EmbeddingService, GcpEmbeddingService};
use crate::{
    models::search::SearchRequest,
    services::payload_generator::build_beckn_payload,
    state::AppState,
    utils::{
        empeding::{cosine_similarity, job_text_for_embedding, profile_text_for_embedding},
        hash::generate_query_hash,
        http_client::post_json,
        search::matches_query_dynamic,
    },
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
) -> Result<Json<JsonValue>, (StatusCode, Json<JsonValue>)> {
    let start = Instant::now();
    let message_id = format!("msg-{}", Uuid::new_v4());
    let txn_id = format!("txn-{}", Uuid::new_v4());

    let query_hash = generate_query_hash(&req.message);
    let pattern = format!("search:{}:*", query_hash);
    info!("Looking for Redis keys with pattern: {}", pattern);

    // --- Get cached search results ---
    let cached_results = match app_state.redis_pool.get().await {
        Ok(mut conn) => {
            let mut stream = conn
                .scan_match::<String, String>(pattern.clone())
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "Failed to scan Redis",
                            "details": e.to_string()
                        })),
                    )
                })?;

            let mut keys = vec![];
            while let Some(key) = stream.next_item().await {
                keys.push(key);
            }
            drop(stream);

            let mut results = vec![];
            for key in keys {
                match conn.get::<String, String>(key.clone()).await {
                    Ok(value) => match serde_json::from_str::<JsonValue>(&value) {
                        Ok(json_value) => results.push(json_value),
                        Err(_) => error!("Failed to parse cached value for key: {}", key),
                    },
                    Err(e) => error!("Redis get error for key {}: {}", key, e),
                }
            }
            results
        }
        Err(e) => {
            error!("Failed to get Redis connection from pool: {:?}", e);
            vec![]
        }
    };

    // --- Cache txn_id -> query_hash for on_search mapping ---
    if let Ok(mut conn) = app_state.redis_pool.get().await {
        let txn_key = format!("txn_to_query:{}", txn_id);
        let _: () = conn
            .set_ex::<_, _, ()>(&txn_key, &query_hash, app_state.config.cache.txn_ttl_secs)
            .await
            .unwrap_or_else(|e| error!("Failed to cache txn_id: {:?}", e));
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

    // --- Throttle BAP calls ---
    let should_call_bap = match app_state.redis_pool.get().await {
        Ok(mut conn) => {
            let last_call_key = format!("last_call:{}", query_hash);
            match conn.exists::<_, bool>(&last_call_key).await {
                Ok(exists) if exists => {
                    let secs = app_state.config.cache.throttle_secs;
                    info!(
                        ": Skipping BAP call (already called within last {} {})",
                        if secs % 60 == 0 { secs / 60 } else { secs },
                        if secs % 60 == 0 { "min" } else { "secs" }
                    );
                    false
                }
                _ => {
                    let _: () = conn
                        .set_ex::<_, _, ()>(
                            &last_call_key,
                            "1",
                            app_state.config.cache.throttle_secs,
                        )
                        .await
                        .unwrap_or_default();
                    true
                }
            }
        }
        Err(e) => {
            error!("Failed to get Redis connection for throttle check: {:?}", e);
            true
        }
    };

    if should_call_bap {
        info!(
            ": Sending search request to BAP adapter at: {}",
            adapter_url
        );
        let payload_clone = payload.clone();
        tokio::spawn(async move {
            if let Err(e) = post_json(&adapter_url, payload_clone).await {
                error!("❌ Failed to send search to BAP adapter: {}", e);
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
        return Ok(Json(serde_json::json!({ "results": cached_results })));
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

    // --- Get a Redis connection from the pool ---
    match app_state.redis_pool.get().await {
        Ok(mut conn) => {
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
        }
        Err(e) => {
            error!("❌ Failed to get Redis connection from pool: {:?}", e);
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

    let mut conn = match app_state.redis_pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!(target: "cron", "❌ Failed to get Redis connection: {:?}", e);
            return Json(AckResponse {
                message: AckStatus {
                    ack: Ack { status: "ACK" },
                },
            });
        }
    };

    // Create embedding service instance
    let embedding_service = GcpEmbeddingService;

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
            let existing_providers = store_data
                .pointer_mut("/message/catalog/providers")
                .and_then(|p| p.as_array_mut());

            if let Some(existing) = existing_providers {
                for mut provider in new_providers.clone() {
                    if let Some(items) = provider.get_mut("items").and_then(|j| j.as_array_mut()) {
                        for job in items.iter_mut() {
                            let text = job_text_for_embedding(job);

                            if text.trim().is_empty() {
                                info!(
                                    "⚠️ Job text is empty, skipping embedding for job: {:?}",
                                    job["id"]
                                );
                                continue;
                            }

                            match embedding_service
                                .get_embedding(&text, &mut conn, app_state)
                                .await
                            {
                                Ok(embedding) => {
                                    job.as_object_mut().unwrap().insert(
                                        "embedding".to_string(),
                                        serde_json::json!(embedding),
                                    );
                                }
                                Err(e) => error!("❌ Failed embedding: {:?}", e),
                            }
                        }
                    }
                    existing.push(provider);
                }
            }
        }

        // --- Pagination info ---
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
            bpp_id, current_page, limit, total_count
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
                current_page, total_count, next_page
            );

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

            let message = json!({
                "intent": intent,
                "pagination": {
                    "page": next_page,
                    "limit": limit
                },
                "options": { "brief": false }
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
                    next_page, txn_id, e
                );
            } else {
                info!(
                    target: "cron",
                    "📨 Successfully requested next_page = {} for txn_id={}",
                    next_page, txn_id
                );
            }
        } else {
            let latest_key = "cron_txn:latest";
            if let Err(e) = conn.set::<_, _, ()>(latest_key, &txn_id).await {
                error!(target: "cron", "❌ Failed to store latest cron txn_id: {:?}", e);
            } else {
                info!(target: "cron", "✅ Updated latest cron transaction to {}", txn_id);
            }

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
    let mut conn = match app_state.redis_pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("❌ Failed to get Redis connection: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to connect to Redis" })),
            ));
        }
    };

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

    let provider_filter = req.provider.as_ref().map(|s| s.to_lowercase());
    let role_filters: Vec<String> = req
        .role
        .as_ref()
        .map(|r| r.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();
    let query_filter = req.query.as_ref().map(|s| s.to_lowercase());
    let primary_filters: Vec<String> = req
        .primary_filters
        .as_ref()
        .map(|r| r.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();

    // 👉 Compute profile embedding if profile exists
    let profile_embedding: Option<Vec<f32>> = if let Some(profile) = &req.profile {
        let profile_text = profile_text_for_embedding(profile);
        info!("Profile text for embedding: {}", profile_text);

        match GcpEmbeddingService
            .get_embedding(&profile_text, &mut conn, &app_state)
            .await
        {
            Ok(vec) => Some(vec),
            Err(e) => {
                error!("Failed to get embedding: {:?}", e);
                None
            }
        }
    } else {
        None
    };

    let mut seen_ids = HashSet::new();
    let mut flat_items = vec![];

    for key in keys {
        if let Ok(Some(payload_str)) = conn.get::<_, Option<String>>(&key).await {
            if let Ok(payload_json) = serde_json::from_str::<JsonValue>(&payload_str) {
                if let Some(providers) = payload_json
                    .pointer("/message/catalog/providers")
                    .and_then(|p| p.as_array())
                {
                    for provider in providers.iter() {
                        let provider_name = provider
                            .get("descriptor")
                            .and_then(|d| d.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_lowercase();

                        // Provider filter
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

                                // Primary filter
                                if !primary_filters.is_empty()
                                    && !primary_filters.iter().any(|pf| role_name.contains(pf))
                                {
                                    continue;
                                }

                                // Role filter
                                if !role_filters.is_empty()
                                    && !role_filters
                                        .iter()
                                        .any(|rf| item_roles.iter().any(|r| r.contains(rf)))
                                {
                                    continue;
                                }

                                // Query filter
                                if let Some(ref qf) = query_filter {
                                    if !matches_query_dynamic(&provider_name, item, qf) {
                                        continue;
                                    }
                                }

                                // Compute match_score
                                let mut match_score = 0u8;
                                if let Some(ref profile_emb) = profile_embedding {
                                    if let Some(embedding_json) = item.get("embedding") {
                                        if let Ok(job_emb) = serde_json::from_value::<Vec<f32>>(
                                            embedding_json.clone(),
                                        ) {
                                            match_score = (cosine_similarity(profile_emb, &job_emb)
                                                * 10.0)
                                                .round()
                                                as u8;
                                        }
                                    }
                                }

                                // Prepare item for response (remove embedding)
                                let mut item_obj = item.as_object().cloned().unwrap_or_default();
                                item_obj.remove("embedding");
                                item_obj.insert("match_score".to_string(), json!(match_score));

                                // Deduplicate
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
                                        json!(item_obj),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by match_score descending if profile provided
    if profile_embedding.is_some() {
        flat_items.sort_by(|(_, _, a), (_, _, b)| {
            let sa = a.get("match_score").and_then(|v| v.as_u64()).unwrap_or(0);
            let sb = b.get("match_score").and_then(|v| v.as_u64()).unwrap_or(0);
            sb.cmp(&sa)
        });
    }

    let total_count = flat_items.len();
    let start = (page - 1) * limit;
    let paginated_items = flat_items
        .into_iter()
        .skip(start)
        .take(limit)
        .collect::<Vec<_>>();

    // Group back into payload → providers → items
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
            "message": { "catalog": { "providers": [] } }
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
