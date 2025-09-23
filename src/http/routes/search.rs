use crate::services::search::{handle_search, handle_search_v2};
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/search", post(handle_search))
        .route("/v2/search", post(handle_search_v2))
        .with_state(app_state)
}
