use crate::state::AppState;
use crate::vector::index_store::rebuild_faiss_from_db;
use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tracing::error;

pub async fn rebuild_faiss_service(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<JsonValue>, (StatusCode, Json<JsonValue>)> {
    let dimension = app_state.config.gcp.dimension;

    match rebuild_faiss_from_db(&app_state, dimension).await {
        Ok(_) => Ok(Json(json!({
            "status": "ok",
            "message": "FAISS rebuild completed successfully"
        }))),
        Err(e) => {
            error!("FAISS rebuild failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "status": "error",
                    "message": format!("FAISS rebuild failed: {}", e)
                })),
            ))
        }
    }
}
