use crate::services::search::handle_search;
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/search", post(handle_search))
        .with_state(app_state)
}
