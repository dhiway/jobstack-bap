use crate::{state::AppState, utils::http_client::get_json};
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use serde_json::Value;
use std::sync::Arc;
use urlencoding::encode;

pub async fn call_google_geocode(app_state: &Arc<AppState>, address: &str) -> Result<Value> {
    let base_url = &app_state.config.services.geo_coding.base_url;
    let api_key = &app_state.config.services.geo_coding.api_key;

    let normalized_address = address.trim().to_lowercase();

    let cache_key = format!("geo:google:in:{}", normalized_address);

    let mut conn = app_state.redis_pool.get().await?;

    if let Ok(Some(cached)) = conn.get::<_, Option<String>>(&cache_key).await {
        tracing::info!("🟢 Google Geocode cache HIT");
        let parsed: Value = serde_json::from_str(&cached)?;
        return Ok(parsed);
    }

    tracing::info!("🟡 Google Geocode cache MISS → calling API");

    let url = format!(
        "{}/geocode/json?address={}&region=IN&key={}",
        base_url,
        encode(address),
        api_key
    );

    let headers = reqwest::header::HeaderMap::new();
    let data = get_json(&url, headers).await?;

    let _: () = conn
        .set_ex(&cache_key, serde_json::to_string(&data)?, 60 * 60 * 24 * 30)
        .await?;

    Ok(data)
}
