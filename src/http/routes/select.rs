use crate::services::select::handle_select;
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;
pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/select", post(handle_select))
        .with_state(app_state)
}
