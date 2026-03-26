use crate::services::search::{
    handle_search, handle_search_v2, handle_search_v3, handle_top_results,
};
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;

pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/search", post(handle_search))
        .route("/v2/search", post(handle_search_v2))
        .route("/v3/search", post(handle_search_v3))
        .route("/v1/search/top", post(handle_top_results))
        .with_state(app_state)
}
