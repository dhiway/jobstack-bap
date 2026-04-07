use crate::db::job::NewJob;
use crate::models::core::{Descriptor, Tag, TagItem};
use crate::models::search::{Intent, Item, Options, Pagination, SearchMessage, SearchTopKRequest};
use crate::models::webhook::WebhookPayload;
use crate::services::payload_generator::build_beckn_payload;
use crate::state::AppState;
use crate::utils::hash::hash_json;
use crate::utils::http_client::post_json;
use chrono::Utc;
use redis::AsyncCommands;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;
pub fn matches_query_dynamic(provider_name: &str, item: &JsonValue, qf: &str) -> bool {
    // Split by comma and normalize
    let queries: Vec<String> = qf
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    for q in queries {
        if provider_name.to_lowercase().contains(&q) {
            return true;
        }

        // role / descriptor
        if item
            .pointer("/descriptor/name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // locations
        if let Some(location) = item.get("locations") {
            if location.to_string().to_lowercase().contains(&q) {
                return true;
            }
        }

        // tags.industry
        if item
            .pointer("/tags/industry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.role
        if item
            .pointer("/tags/role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.jobDetails.title
        if item
            .pointer("/tags/jobDetails/title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.jobProviderLocation
        if let Some(loc) = item.pointer("/tags/jobProviderLocation") {
            if loc.to_string().to_lowercase().contains(&q) {
                return true;
            }
        }

        // tags.basicInfo.jobProviderName
        if let Some(loc) = item.pointer("/tags/basicInfo/jobProviderName") {
            if loc.to_string().to_lowercase().contains(&q) {
                return true;
            }
        }
    }

    false
}

pub fn matches_exclude(item: &JsonValue, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }

    let role = item
        .pointer("/tags/role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let industry = item
        .pointer("/tags/industry")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    excludes
        .iter()
        .any(|ex| role.contains(ex) || industry.contains(ex))
}

pub fn extract_jobs_from_on_search(payload: &WebhookPayload, transaction_id: &str) -> Vec<NewJob> {
    let mut jobs = Vec::new();

    let providers = payload
        .message
        .get("catalog")
        .and_then(|c| c.get("providers"))
        .and_then(|p| p.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    for provider in providers {
        let provider_id = match provider.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };

        let items = provider
            .get("items")
            .and_then(|i| i.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        for item in items {
            let job_id = match item.get("id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => continue,
            };

            let beckn_structure = item.clone();

            let hash = hash_json(&beckn_structure);

            jobs.push(NewJob {
                job_id: job_id.to_string(),
                provider_id: provider_id.to_string(),
                transaction_id: transaction_id.to_string(),
                bpp_id: payload.context.bpp_id.clone().unwrap_or_default(),
                bpp_uri: payload.context.bpp_uri.clone().unwrap_or_default(),
                metadata: None,
                beckn_structure: Some(beckn_structure),
                hash,
                last_synced_at: Some(Utc::now()),
            });
        }
    }

    jobs
}

fn build_open_jobs_intent() -> Intent {
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

    Intent {
        item: Some(item),
        provider: None,
        fulfillment: None,
    }
}

fn build_open_jobs_search_message(page: u32, limit: u32) -> SearchMessage {
    SearchMessage {
        intent: build_open_jobs_intent(),
        pagination: Some(Pagination {
            page: Some(page),
            limit: Some(limit),
        }),
        options: Some(Options { breif: Some(false) }),
    }
}

pub async fn send_open_jobs_search(
    app_state: &Arc<AppState>,
    page: u32,
    limit: u32,
    source: &str,
    txn_id: Option<String>,
    bpp_id: Option<&str>,
    bpp_uri: Option<&str>,
) {
    let message = build_open_jobs_search_message(page, limit);

    let (txn_id, is_new_txn) = match txn_id {
        Some(id) => (id, false),
        None => (format!("{}-{}", source, Uuid::new_v4()), true),
    };

    let message_id = format!("msg-{}", Uuid::new_v4());

    let payload = build_beckn_payload(
        &app_state.config,
        &txn_id,
        &message_id,
        &message,
        "search",
        bpp_id,
        bpp_uri,
    );

    if is_new_txn {
        let redis_key = format!("cron_txn:{}", txn_id);
        let metadata = serde_json::json!({
            "source": source,
            "brief": false,
            "all_jobs": true,
            "timestamp": Utc::now(),
        });

        match app_state.redis_pool.get().await {
            Ok(mut conn) => {
                let ttl_secs = app_state.config.cache.txn_ttl_secs;

                let res: Result<(), redis::RedisError> = conn
                    .set_ex(redis_key.clone(), metadata.to_string(), ttl_secs)
                    .await;

                if let Err(e) = res {
                    error!("❌ Failed to store txn metadata in Redis: {:?}", e);
                } else {
                    info!("✅ Stored txn metadata in Redis: {}", redis_key);
                }
            }
            Err(e) => {
                error!("❌ Failed to get Redis connection: {:?}", e);
            }
        }
    }

    let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);
    if let Err(e) = post_json(&adapter_url, payload, None).await {
        error!(
            "❌ Failed to send open jobs search (txn_id={}, page={}): {}",
            txn_id, page, e
        );
    } else {
        info!(
            "📨 Open jobs search sent (txn_id={}, page={}, source={})",
            txn_id, page, source
        );
    }
}

pub fn build_profile_json(req: &SearchTopKRequest) -> JsonValue {
    let mut metadata = serde_json::Map::new();

    // /metadata/role
    if let Some(role) = &req.role {
        metadata.insert("role".to_string(), json!(role));
    }

    // /metadata/industry
    if let Some(industry) = &req.industry {
        metadata.insert("industry".to_string(), json!(industry));
    }

    // /metadata/whoIAm/*
    let mut who_i_am = serde_json::Map::new();

    if let Some(age) = req.age {
        who_i_am.insert("age".to_string(), json!(age));
    }

    if let Some(location) = &req.location {
        let mut location_data = serde_json::Map::new();
        location_data.insert("city".to_string(), json!(location));
        who_i_am.insert("locationData".to_string(), JsonValue::Object(location_data));
    }

    if !who_i_am.is_empty() {
        metadata.insert("whoIAm".to_string(), JsonValue::Object(who_i_am));
    }

    // /metadata/whatIHave/*
    let mut what_i_have = serde_json::Map::new();

    if let Some(iti_specialization) = &req.iti_specialization {
        if !iti_specialization.is_empty() {
            what_i_have.insert("itiSpecialization".to_string(), json!(iti_specialization));
        }
    }

    if let Some(languages) = &req.languages {
        if !languages.is_empty() {
            what_i_have.insert("languagesKnown".to_string(), json!(languages));
        }
    }

    if let Some(highest_qualification) = &req.highest_qualification {
        if !highest_qualification.is_empty() {
            what_i_have.insert("highestEducation".to_string(), json!(highest_qualification));
        }
    }

    if let Some(software_skills) = &req.software_skills {
        if !software_skills.is_empty() {
            what_i_have.insert("softwareSkills".to_string(), json!(software_skills));
        }
    }

    if !what_i_have.is_empty() {
        metadata.insert("whatIHave".to_string(), JsonValue::Object(what_i_have));
    }

    // /metadata/whatIWant/*
    let mut what_i_want = serde_json::Map::new();

    if let Some(preferred_work_mode) = &req.preferred_work_mode {
        if !preferred_work_mode.is_empty() {
            what_i_want.insert("preferredWorkMode".to_string(), json!(preferred_work_mode));
        }
    }

    if let Some(monthly_in_hand) = req.monthly_in_hand {
        what_i_want.insert("monthlyInHandPreferred".to_string(), json!(monthly_in_hand));
    }

    if let Some(work_hours_per_day) = req.work_hours_per_day {
        what_i_want.insert("workHoursPerDay".to_string(), json!(work_hours_per_day));
    }

    if !what_i_want.is_empty() {
        metadata.insert("whatIWant".to_string(), JsonValue::Object(what_i_want));
    }

    json!({
        "metadata": JsonValue::Object(metadata),
         "queryText": req.query_text
    })
}
