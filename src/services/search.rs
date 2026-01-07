use crate::models::search::SearchRequestV2;
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::empeding::{EmbeddingService, GcpEmbeddingService};
use crate::{
    models::search::SearchRequest,
    services::payload_generator::build_beckn_payload,
    state::AppState,
    utils::{
        empeding::{compute_match_score, job_text_for_embedding, profile_text_for_embedding},
        hash::generate_query_hash,
        http_client::post_json,
        search::{matches_exclude, matches_query_dynamic},
    },
};
use axum::{extract::State, http::StatusCode, Json};
use redis::AsyncCommands;
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
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

        // Append or update new providers
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
                // Build a map of existing providers by jobProviderName
                let mut provider_index_map = std::collections::HashMap::new();
                for (i, provider) in existing.iter().enumerate() {
                    if let Some(name) = provider
                        .pointer("/items/0/tags/basicInfo/jobProviderName")
                        .and_then(|v| v.as_str())
                    {
                        provider_index_map.insert(name.to_string(), i);
                    }
                }

                for mut provider in new_providers.clone() {
                    let provider_name = provider
                        .pointer("/items/0/tags/basicInfo/jobProviderName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown Provider")
                        .to_string();

                    if let Some(items) = provider.get_mut("items").and_then(|j| j.as_array_mut()) {
                        for job in items.iter_mut() {
                            let job_id = job
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown Job")
                                .to_string();

                            let text = job_text_for_embedding(job, &app_state.config);

                            if text.trim().is_empty() {
                                info!(
                                    target: "cron",
                                    "⚠️ Skipping embedding: provider='{}', job_id='{}', reason='empty text'",
                                    provider_name,
                                    job_id
                                );
                                continue;
                            }

                            info!(
                                target: "cron",
                                "🔹 Generating embedding: provider='{}', job_id='{}', text_len={}",
                                provider_name,
                                job_id,
                                text.len()
                            );

                            match embedding_service
                                .get_embedding(&text, &mut conn, app_state)
                                .await
                            {
                                Ok(embedding) => {
                                    let embedding_len = embedding.len();

                                    if let Some(obj) = job.as_object_mut() {
                                        obj.insert(
                                            "embedding".to_string(),
                                            serde_json::json!(embedding),
                                        );
                                    }

                                    let is_stored = job.get("embedding").is_some();
                                    info!(
                                        target: "cron",
                                        "✅ Embedding stored: provider=  {}, job_id='{}', embedding_len={}, stored_in_job={}",
                                        provider_name,
                                        job_id,
                                        embedding_len,
                                        is_stored
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        target: "cron",
                                        "❌ Failed embedding: provider='{}', job_id='{}', error={:?}",
                                        provider_name,
                                        job_id,
                                        e
                                    );
                                }
                            }
                        }
                    } else {
                        info!(
                            target: "cron",
                            "⚠️ No items found for provider='{}', skipping embedding",
                            provider_name
                        );
                    }

                    // Insert or update provider
                    if let Some(&idx) = provider_index_map.get(&provider_name) {
                        // Merge items for existing provider instead of replacing the whole thing
                        if let Some(existing_items) = existing[idx]
                            .get_mut("items")
                            .and_then(|v| v.as_array_mut())
                        {
                            if let Some(new_items) =
                                provider.get("items").and_then(|v| v.as_array())
                            {
                                // Build set of existing job_ids to avoid duplicates
                                let existing_job_ids: std::collections::HashSet<String> =
                                    existing_items
                                        .iter()
                                        .filter_map(|job| {
                                            job.get("id")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string())
                                        })
                                        .collect();

                                for new_job in new_items {
                                    if let Some(job_id) = new_job.get("id").and_then(|v| v.as_str())
                                    {
                                        if !existing_job_ids.contains(job_id) {
                                            existing_items.push(new_job.clone());
                                        } else {
                                            // Optional: update embedding if exists
                                            if let Some(existing_job) =
                                                existing_items.iter_mut().find(|j| {
                                                    j.get("id").and_then(|v| v.as_str())
                                                        == Some(job_id)
                                                })
                                            {
                                                if let Some(new_embedding) =
                                                    new_job.get("embedding")
                                                {
                                                    if let Some(obj) = existing_job.as_object_mut()
                                                    {
                                                        obj.insert(
                                                            "embedding".to_string(),
                                                            new_embedding.clone(),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        existing.push(provider);
                    }
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
                Some(&bpp_id),
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
    // ✅ Initialize string similarity cache

    let mut string_sim_cache: HashMap<(String, String), f32> = HashMap::new();

    // ✅ Get latest txn_id
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

    // ✅Fetch all BPP results for this txn_id
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
    let exclude_filters: Vec<String> = req
        .exclude
        .as_ref()
        .map(|e| e.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();

    // ✅ Compute embedding for profile
    let profile_embedding: Option<Vec<f32>> = if let Some(profile) = &req.profile {
        let profile_text = profile_text_for_embedding(profile, &app_state.config);
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
    let mut flat_items = Vec::new();

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

                                // Filters
                                if !primary_filters.is_empty()
                                    && !primary_filters.iter().any(|pf| role_name.contains(pf))
                                {
                                    continue;
                                }
                                if matches_exclude(item, &exclude_filters) {
                                    continue;
                                }

                                if !role_filters.is_empty()
                                    && !role_filters
                                        .iter()
                                        .any(|rf| item_roles.iter().any(|r| r.contains(rf)))
                                {
                                    continue;
                                }

                                if let Some(ref qf) = query_filter {
                                    if !matches_query_dynamic(&provider_name, item, qf) {
                                        continue;
                                    }
                                }

                                // ✅ Compute match_score
                                let mut match_score = 0u8;
                                if let Some(ref profile_emb) = profile_embedding {
                                    if let Some(embedding_json) = item.get("embedding") {
                                        if let Ok(job_emb) = serde_json::from_value::<Vec<f32>>(
                                            embedding_json.clone(),
                                        ) {
                                            // fix: avoid temporary drop
                                            let empty_json = serde_json::json!({});
                                            let profile_meta =
                                                req.profile.as_ref().unwrap_or(&empty_json);
                                            let profile_norm = profile_embedding
                                                .as_ref()
                                                .map(|v| {
                                                    v.iter().map(|x| x * x).sum::<f32>().sqrt()
                                                })
                                                .unwrap_or(0.0);

                                            let score = compute_match_score(
                                                profile_emb,
                                                profile_norm,
                                                &job_emb,
                                                job_emb.iter().map(|x| x * x).sum::<f32>().sqrt(), // job norm
                                                profile_meta,
                                                &item,
                                                &app_state.config,
                                                &mut string_sim_cache,
                                            );

                                            match_score = (score * 10.0).round() as u8;
                                        }
                                    }
                                }

                                // ✅ Prepare cleaned item
                                let mut item_obj = item.as_object().cloned().unwrap_or_default();
                                item_obj.remove("embedding");
                                item_obj.insert("match_score".to_string(), json!(match_score));

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

    // ✅ Global sort by match_score DESC (ensure correct ordering)
    if profile_embedding.is_some() {
        flat_items.sort_by(|(_, _, a), (_, _, b)| {
            let sa = a.get("match_score").and_then(|v| v.as_u64()).unwrap_or(0);
            let sb = b.get("match_score").and_then(|v| v.as_u64()).unwrap_or(0);
            sb.cmp(&sa) // descending
        });
    }

    // ✅ Pagination after sorting
    let total_count = flat_items.len();
    let start = (page - 1) * limit;
    if start >= total_count {
        return Ok(Json(json!({
            "pagination": {
                "page": page,
                "limit": limit,
                "totalCount": total_count
            },
            "results": []
        })));
    }

    let end = std::cmp::min(start + limit, total_count);
    let paginated_items = flat_items[start..end].to_vec();

    // ✅ Rebuild ONDC-compatible response
    let results: Vec<JsonValue> = paginated_items
        .into_iter()
        .map(|(context, provider, item)| {
            json!({
                "context": context,
                "message": {
                    "catalog": {
                        "providers": [
                            {
                                "descriptor": provider["descriptor"].clone(),
                                "id": provider.get("id").cloned().unwrap_or(json!(null)),
                                "fulfillments": provider.get("fulfillments").cloned().unwrap_or(json!([])),
                                "locations": provider.get("locations").cloned().unwrap_or(json!([])),
                                "items": [item]
                            }
                        ]
                    }
                }
            })
        })
        .collect();

    // ✅ Final response
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
