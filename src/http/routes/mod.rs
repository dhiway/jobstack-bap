pub mod events;
pub mod job;
pub mod search;
pub mod select;
pub mod status;
pub mod webhook;
use crate::models::webhook::HealthResponse;
use crate::state::AppState;
use axum::{response::IntoResponse, routing::get, Json, Router};
use chrono::Utc;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
async fn health_check() -> impl IntoResponse {
    let response = HealthResponse {
        status: "OK",
        timestamp: Utc::now().to_rfc3339(),
    };

    Json(response)
}

pub fn create_routes(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/", get(health_check))
        .nest("/api", search::routes(app_state.clone()))
        .nest("/api", job::routes(app_state.clone()))
        .nest("/api", select::routes(app_state.clone()))
        .nest("/api", status::routes(app_state.clone()))
        .nest("/api", events::routes(app_state.clone()))
        .merge(webhook::routes(app_state))
        .layer(cors)
}
