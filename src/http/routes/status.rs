use crate::services::status::handle_status;
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/status", post(handle_status))
        .with_state(app_state)
}
