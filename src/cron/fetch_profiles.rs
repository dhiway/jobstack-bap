use crate::cron::job_profile_match;
use crate::db::profiles::{delete_stale_profiles, store_profiles, NewProfile};
use crate::state::AppState;
use crate::utils::http_client::get_json;
use chrono::{DateTime, Utc};
use reqwest::header;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct ProfilesApiResponse {
    pub data: Vec<ApiProfile>,
    pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct ApiProfile {
    pub id: String,

    #[serde(rename = "userId")]
    pub user_id: String,

    #[serde(rename = "type")]
    pub r#type: String,

    pub metadata: Value,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
#[derive(Debug, Deserialize)]
struct Pagination {
    #[serde(rename = "totalCount")]
    pub total_count: u32,
}
pub fn compute_profile_hash(profile: &ApiProfile) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(&profile.id);
    hasher.update(&profile.user_id);
    hasher.update(&profile.r#type);
    hasher.update(serde_json::to_string(&profile.metadata).unwrap());
    hasher.update(&profile.created_at);
    hasher.update(&profile.updated_at);

    let result = hasher.finalize();
    hex::encode(result)
}

pub fn build_beckn_structure(profile_id: &str, metadata: &Value) -> Value {
    let name = metadata
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    json!({
        "id": profile_id,
        "descriptor": {
            "name": name
        },
        "tags": {
            "profile": metadata
        }
    })
}

pub async fn run(app_state: Arc<AppState>) {
    info!(target: "cron", "🔄 Starting fetch profiles cron");
    let sync_started_at: DateTime<Utc> = Utc::now();
    info!(target: "cron", "🕒 Sync started at {}", sync_started_at);

    let base_url = &app_state.config.services.seeker.base_url;
    let api_key = &app_state.config.services.seeker.api_key;

    let mut page = 1;
    let limit = 100;
    let mut sync_completed = true;

    loop {
        let url = format!("{}/profile/all?page={}&limit={}", base_url, page, limit);

        let mut headers = header::HeaderMap::new();
        headers.insert("x-api-key", header::HeaderValue::from_str(api_key).unwrap());

        let response: ProfilesApiResponse = loop {
            match get_json(&url, headers.clone()).await {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(parsed) => break parsed,
                    Err(e) => {
                        error!("❌ Failed to parse profiles response: {:?}", e);
                        return;
                    }
                },
                Err(e) => {
                    let error_string = e.to_string();
                    if error_string.contains("429") {
                        let mut retry_seconds = 20;

                        if let Some(pos) = error_string.find("retry in ") {
                            let slice = &error_string[pos + 9..];
                            if let Some(end) = slice.find(" ") {
                                if let Ok(sec) = slice[..end].parse::<u64>() {
                                    retry_seconds = sec;
                                }
                            }
                        }

                        error!(
                            "⚠️ Rate limited. Waiting {} seconds before retrying page {}...",
                            retry_seconds, page
                        );

                        sleep(Duration::from_secs(retry_seconds)).await;
                        continue;
                    }

                    error!("❌ Failed to fetch profiles: {:?}", e);
                    return;
                }
            }
        };

        if response.data.is_empty() {
            break;
        }

        let profiles: Vec<NewProfile> = response
            .data
            .iter()
            .map(|p| {
                let beckn_structure = build_beckn_structure(&p.id, &p.metadata);

                NewProfile {
                    profile_id: p.id.clone(),
                    user_id: p.user_id.clone(),
                    r#type: p.r#type.clone(),
                    metadata: Some(p.metadata.clone()),
                    beckn_structure: Some(beckn_structure),
                    hash: compute_profile_hash(p),
                    last_synced_at: sync_started_at,
                }
            })
            .collect();

        if let Err(e) = store_profiles(&app_state.db_pool, &profiles).await {
            error!("❌ Failed to store profiles: {:?}", e);
            sync_completed = false;
            break;
        }

        let fetched = page * limit;
        if fetched >= response.pagination.total_count {
            break;
        }

        page += 1;
        sleep(Duration::from_millis(300)).await;
    }

    if sync_completed {
        match delete_stale_profiles(&app_state.db_pool, sync_started_at).await {
            Ok(count) => {
                info!(target: "cron", "🧹 Deleted {} stale profiles", count);
            }
            Err(e) => {
                error!("❌ Failed to delete stale profiles: {:?}", e);
            }
        }

        info!(target: "cron", "🔗 Triggering job-profile match scoring...");

        tokio::spawn({
            let state = app_state.clone();
            async move {
                job_profile_match::run(state).await;
            }
        });
    } else {
        info!(
            target: "cron",
            "⚠️ Profile sync did not complete successfully — skipping stale profile cleanup & match scoring"
        );
    }

    info!(target: "cron", "✅ Fetch profiles cron completed successfully");
}
