use crate::services::webhook::{webhook_handler, webhook_handler_profiles};
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/webhook/{action}", post(webhook_handler))
        .route("/webhook/profiles/{action}", post(webhook_handler_profiles))
        .with_state(app_state)
}
