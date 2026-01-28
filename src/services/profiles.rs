use crate::db::profiles::fetch_beckn_profile_items;
use crate::models::core::Context;
use crate::services::payload_generator::build_profile_beckn_response;
use crate::state::AppState;
use crate::utils::profiles::{build_profiles_catalog, extract_pagination};
use serde_json::Value;

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
