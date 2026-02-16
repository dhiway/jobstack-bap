use crate::cron::fetch_profiles::{
    build_beckn_structure, compute_profile_hash, ProfilesApiResponse,
};
use crate::cron::job_profile_match;
use crate::db::profiles::fetch_beckn_profile_items;
use crate::db::profiles::{store_profiles, NewProfile};
use crate::models::core::Context;
use crate::services::payload_generator::build_profile_beckn_response;
use crate::state::AppState;
use crate::utils::http_client::get_json;
use crate::utils::profiles::{build_profiles_catalog, extract_pagination};
use chrono::Utc;
use reqwest::header;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};
pub async fn handle_search_profiles(
    context: Context,
    message: Value,
    state: &AppState,
) -> anyhow::Result<Value> {
    let pagination = extract_pagination(&message);

    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(10);

    let result = fetch_beckn_profile_items(&state.db_pool, pagination).await?;

    let catalog = build_profiles_catalog(
        result.items.clone(),
        &state.config,
        page,
        limit,
        result.total,
    );
    let response = build_profile_beckn_response(&state.config, context, &catalog);

    Ok(response)
}

pub async fn sync_profile_by_id(state: &Arc<AppState>, profile_id: &str) -> anyhow::Result<()> {
    let base_url = &state.config.services.seeker.base_url;
    let api_key = &state.config.services.seeker.api_key;

    let url = format!(
        "{}/profile/all?page=1&limit=30&profileId={}",
        base_url, profile_id
    );

    let mut headers = header::HeaderMap::new();
    headers.insert("x-api-key", header::HeaderValue::from_str(api_key)?);

    let response_value = get_json(&url, headers).await.map_err(|e| {
        error!("❌ Failed to fetch profile {}: {:?}", profile_id, e);
        e
    })?;

    let response: ProfilesApiResponse = serde_json::from_value(response_value).map_err(|e| {
        error!("❌ Failed to parse profile {}: {:?}", profile_id, e);
        e
    })?;

    if response.data.is_empty() {
        info!("⚠️ No profile found for id={}", profile_id);
        return Ok(());
    }
    let profile = &response.data[0];

    let beckn_structure = build_beckn_structure(&profile.id, &profile.metadata);

    let profile = &response.data[0];

    let new_profile = NewProfile {
        profile_id: profile.id.clone(),
        user_id: profile.user_id.clone(),
        r#type: profile.r#type.clone(),
        metadata: Some(profile.metadata.clone()),
        beckn_structure: Some(beckn_structure),
        hash: compute_profile_hash(profile),
        last_synced_at: Utc::now(),
    };

    store_profiles(&state.db_pool, &[new_profile]).await?;

    info!("✅ Profile {} synced successfully", profile_id);

    tokio::spawn({
        let state_clone = state.clone();
        async move {
            job_profile_match::run(state_clone).await;
        }
    });
    Ok(())
}
