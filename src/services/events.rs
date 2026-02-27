use crate::events::publisher::publish_event;
use crate::models::{
    core::ErrorResponse,
    events::{AppEvent, EventRequest, EventResponse},
};
use crate::state::AppState;
use crate::workers::redis_event_worker::STREAM_NAME;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub async fn handle_events(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EventRequest>,
) -> Result<impl IntoResponse, Response> {
    let event = AppEvent {
        id: Uuid::new_v4(),
        event_type: request.event_type,
        payload: request.payload,
        created_at: Utc::now(),
    };

    let mut conn = match state.redis_pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Redis connection failed: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Redis connection failed".to_string(),
                }),
            )
                .into_response());
        }
    };

    if let Err(e) = publish_event(&mut conn, STREAM_NAME, &event).await {
        tracing::error!("Failed to publish event: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to publish event".to_string(),
            }),
        )
            .into_response());
    }

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            event_id: event.id,
            status: "ok".to_string(),
            stream: STREAM_NAME.to_string(),
            created_at: event.created_at,
        }),
    ))
}
