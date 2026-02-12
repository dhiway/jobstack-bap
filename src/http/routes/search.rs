use crate::services::search::{handle_search, handle_search_v2, handle_search_v3};
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;

pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/search", post(handle_search))
        .route("/v2/search", post(handle_search_v2))
        .route("/v3/search", post(handle_search_v3))
        .with_state(app_state)
}
