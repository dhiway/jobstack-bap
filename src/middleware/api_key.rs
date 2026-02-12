use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

pub async fn api_key_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let expected_key = &state.config.auth.x_api_key;

    let provided_key = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());

    match provided_key {
        Some(key) if key == expected_key => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Unauthorized",
                "message": "Invalid or missing X-API-KEY"
            })),
        )
            .into_response(),
    }
}
