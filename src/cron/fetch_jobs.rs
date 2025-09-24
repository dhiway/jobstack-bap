use crate::models::core::{Descriptor, Tag, TagItem};
use crate::models::search::{Intent, Item, Options, Pagination, SearchMessage};
use crate::services::payload_generator::build_beckn_payload;
use crate::state::AppState;
use crate::utils::http_client::post_json;
use chrono::Utc;
use redis::AsyncCommands;
use tracing::{error, info};
use uuid::Uuid;

pub async fn run(app_state: AppState) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 Starting fetch jobs cron.             ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    // ✅ Build intent with item + status tag
    let item = Item {
        descriptor: None,
        tags: Some(vec![Tag {
            descriptor: Descriptor {
                code: "status".to_string(),
                name: "Status".to_string(),
            },
            list: vec![TagItem {
                descriptor: Descriptor {
                    code: "status".to_string(),
                    name: "Status".to_string(),
                },
                value: "open".to_string(),
            }],
        }]),
    };

    // Intent
    let intent = Intent {
        item: Some(item),
        provider: None,
        fulfillment: None,
    };

    let message = SearchMessage {
        intent,
        pagination: Some(Pagination {
            page: Some(1),
            limit: Some(1000),
        }),
        options: Some(Options { breif: Some(false) }),
    };

    // Generate unique IDs for this cron run
    let message_id = format!("msg-{}", Uuid::new_v4());
    let txn_id = format!("cron-{}", Uuid::new_v4());

    // Build Beckn payload
    let payload = build_beckn_payload(
        &app_state.config,
        &txn_id,
        &message_id,
        &message,
        "search",
        None,
        None,
    );

    // Metadata to store in Redis for additional info
    let redis_key = format!("cron_txn:{}", txn_id);
    let mut conn = app_state.redis_conn.lock().await;
    let metadata = serde_json::json!({
        "source": "cron",
        "brief": false,
        "all_jobs": true,
        "timestamp": Utc::now()
    });

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

    // Send to BAP adapter
    let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);
    if let Err(e) = post_json(&adapter_url, payload).await {
        error!(target: "cron", "❌ Failed to send search to BAP adapter: {}", e);
    }
}
