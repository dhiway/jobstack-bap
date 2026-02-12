use crate::services::events::handle_events;
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;
pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/event", post(handle_events))
        .with_state(app_state)
}
