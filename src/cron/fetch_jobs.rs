use crate::{
    models::search::{Intent, Options, Pagination, SearchMessage},
    services::payload_generator::build_beckn_payload,
    state::AppState,
    utils::http_client::post_json,
};
use redis::AsyncCommands;
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

pub async fn run(app_state: AppState) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 Starting fetch jobs cron.             ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    // ──────────────────────────────────────────────
    // Prepare the initial search message with pagination and options
    let message = SearchMessage {
        intent: Intent {
            item: None,
            provider: None,
            fulfillment: None,
        },
        pagination: Some(Pagination {
            page: Some(1),
            limit: Some(1000),
        }),
        options: Some(Options { breif: Some(false) }),
    };

    // ──────────────────────────────────────────────
    // Generate unique IDs for this cron run
    let message_id = format!("msg-{}", Uuid::new_v4());
    let txn_id = format!("cron-{}", Uuid::new_v4());

    // ──────────────────────────────────────────────
    // Build the Beckn payload for the search request
    let payload = build_beckn_payload(
        &app_state.config,
        &txn_id,
        &message_id,
        &message,
        "search",
        None,
        None,
    );

    // ──────────────────────────────────────────────
    // Metadata to store in Redis for additional info
    let metadata = json!({
        "source": "cron",
        "brief": false,
        "all_jobs": true,
        "timestamp": chrono::Utc::now()
    });

    // Redis key for this specific cron transaction
    let redis_key = format!("cron_txn:{}", txn_id);
    let mut conn = app_state.redis_conn.lock().await;

    // Store transaction metadata in Redis with TTL
    if let Err(e) = conn
        .set_ex::<_, _, ()>(
            &redis_key,
            metadata.to_string(),
            app_state.config.cache.txn_ttl_secs,
        )
        .await
    {
        error!(target: "cron", "❌ Failed to store cron txn metadata: {:?}", e);
    }

    // ──────────────────────────────────────────────
    // Update a separate key to always point to the latest cron transaction
    let latest_key = "cron_txn:latest";
    if let Err(e) = conn
        .set_ex::<_, _, ()>(latest_key, &txn_id, app_state.config.cache.txn_ttl_secs)
        .await
    {
        error!(target: "cron", "❌ Failed to store latest cron txn_id: {:?}", e);
    } else {
        info!(target: "cron", "✅ Updated latest cron transaction to {}", txn_id);
    }

    // ──────────────────────────────────────────────
    // Send the search request payload to the BAP adapter
    let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);
    if let Err(e) = post_json(&adapter_url, payload).await {
        error!(target: "cron", "❌ Failed to send search to BAP adapter: {}", e);
    }
}
