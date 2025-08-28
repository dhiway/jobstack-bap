use crate::config::AppConfig;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};

fn generate_context(
    config: &AppConfig,
    txn_id: &str,
    message_id: &str,
    action: &str,
    bpp_id: Option<&str>,
    bpp_uri: Option<&str>,
) -> Value {
    let now = Utc::now().to_rfc3339();

    let mut context = json!({
        "action": action,
        "bap_id": config.bap.id,
        "bap_uri": config.bap.bap_uri,
        "domain": config.bap.domain,
        "message_id": message_id,
        "transaction_id": txn_id,
        "timestamp": now,
        "ttl": "PT30S",
        "version": config.bap.version
    });

    if let Some(id) = bpp_id {
        context["bpp_id"] = json!(id);
    }
    if let Some(uri) = bpp_uri {
        context["bpp_uri"] = json!(uri);
    }

    context
}

pub fn build_beckn_payload(
    config: &AppConfig,
    txn_id: &str,
    message_id: &str,
    message: &impl Serialize,
    action: &str,
    bpp_id: Option<&str>,
    bpp_uri: Option<&str>,
) -> Value {
    let context = generate_context(config, txn_id, message_id, action, bpp_id, bpp_uri);

    json!({
        "context": context,
        "message": message,
    })
}
