use crate::services::status::handle_status;
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;
pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/status", post(handle_status))
        .with_state(app_state)
}
