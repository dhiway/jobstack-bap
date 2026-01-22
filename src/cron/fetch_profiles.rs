use crate::db::profiles::{delete_stale_profiles, store_profiles, NewProfile};
use crate::state::AppState;
use crate::utils::http_client::get_json;
use chrono::{DateTime, Utc};
use reqwest::header;
use tracing::{error, info};

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ProfilesApiResponse {
    data: Vec<ApiProfile>,
    pagination: Pagination,
}

#[derive(Debug, Deserialize)]
struct ApiProfile {
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
fn compute_profile_hash(profile: &ApiProfile) -> String {
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

pub async fn run(app_state: AppState) {
    info!(target: "cron", "🔄 Starting fetch profiles cron");
    let sync_started_at: DateTime<Utc> = Utc::now();
    info!(target: "cron", "🕒 Sync started at {}", sync_started_at);

    let base_url = &app_state.config.services.seeker.base_url;
    let api_key = &app_state.config.services.seeker.api_key;

    let mut page = 1;
    let limit = 50;

    loop {
        let url = format!("{}/profile/all?page={}&limit={}", base_url, page, limit);

        let mut headers = header::HeaderMap::new();
        headers.insert("x-api-key", header::HeaderValue::from_str(api_key).unwrap());

        let response: ProfilesApiResponse = match get_json(&url, headers).await {
            Ok(v) => serde_json::from_value(v).unwrap(),
            Err(e) => {
                error!("❌ Failed to fetch profiles: {:?}", e);
                break;
            }
        };

        if response.data.is_empty() {
            break;
        }

        let profiles: Vec<NewProfile> = response
            .data
            .iter()
            .map(|p| NewProfile {
                profile_id: p.id.clone(),
                user_id: p.user_id.clone(),
                r#type: p.r#type.clone(),
                metadata: Some(p.metadata.clone()),
                beckn_structure: None,
                hash: compute_profile_hash(p),
                last_synced_at: sync_started_at,
            })
            .collect();

        if let Err(e) = store_profiles(&app_state.db_pool, &profiles).await {
            error!("❌ Failed to store profiles: {:?}", e);
            break;
        }

        let fetched = page * limit;
        if fetched >= response.pagination.total_count {
            break;
        }

        page += 1;
    }

    match delete_stale_profiles(&app_state.db_pool, sync_started_at).await {
        Ok(count) => {
            info!(
                target: "cron",
                "🧹 Deleted {} stale profiles",
                count
            );
        }
        Err(e) => {
            error!("❌ Failed to delete stale profiles: {:?}", e);
        }
    }

    info!(target: "cron", "✅ Fetch profiles cron completed successfully");
}
