use crate::services::webhook::webhook_handler;
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/webhook/{action}", post(webhook_handler))
        .with_state(app_state)
}
