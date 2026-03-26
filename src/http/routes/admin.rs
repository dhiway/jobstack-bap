use crate::services::admin::rebuild_faiss_service;
use crate::state::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;
pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/faiss/rebuild", post(rebuild_faiss_service))
        .with_state(app_state)
}
