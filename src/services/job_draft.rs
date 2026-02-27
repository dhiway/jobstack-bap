use crate::db::job_draft::{
    delete_draft_application, get_draft_applications, upsert_draft_application, CreateJobDraft,
    UpdateJobDraft,
};
use crate::{
    models::job_apply::{DraftApplicationsQuery, DraftRequest},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
pub async fn create_user_draft_application(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<DraftRequest>,
) -> Result<impl IntoResponse, Response> {
    let user_id = req
        .message
        .order
        .fulfillments
        .as_ref()
        .and_then(|fulfillments| fulfillments.get(0))
        .and_then(|f| f.customer.as_ref())
        .map(|c| c.person.id.clone())
        .unwrap_or_else(|| "".to_string());

    let job_id = req
        .message
        .order
        .items
        .get(0)
        .map(|item| item.id.clone())
        .unwrap_or_else(|| "".to_string());
    let bpp_id = req.context.bpp_id.clone();
    let bpp_uri = req.context.bpp_uri.clone();

    let existing =
        match get_draft_applications(&app_state.db_pool, Some(&user_id), Some(&job_id)).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("DB error: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Database error"})),
                )
                    .into_response());
            }
        };

    if let Some(application) = existing.into_iter().next() {
        let response_body = json!({
            "message": "User has already drafted for this job",
            "application": application
        });
        return Ok((StatusCode::OK, axum::Json(response_body)));
    }
    let metadata = match serde_json::to_value(&req.message) {
        Ok(val) => Some(val),
        Err(e) => {
            tracing::error!("Failed to convert message to JSON: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Invalid message format"})),
            )
                .into_response());
        }
    };

    let draft_data = CreateJobDraft {
        user_id,
        job_id,
        bpp_id,
        bpp_uri,
        metadata,
        id: None,
    };

    // Otherwise, insert  new draft application
    match upsert_draft_application(&app_state.db_pool, draft_data).await {
        Ok(_) => {
            let response_body = json!({
                "message": "Draft application saved successfully"
            });
            Ok((StatusCode::OK, Json(response_body)))
        }
        Err(e) => {
            tracing::error!("DB error while saving draft: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to save draft application"})),
            )
                .into_response())
        }
    }
}

pub async fn get_user_draft_applications(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DraftApplicationsQuery>,
) -> impl IntoResponse {
    let user_id = &params.user_id;

    let result: Result<_, _> =
        get_draft_applications(&app_state.db_pool, Some(user_id), None).await;

    match result {
        Ok(applications) => {
            let apps_json: Vec<_> = applications
                .into_iter()
                .map(|app| {
                    json!({
                        "id":app.id,
                        "job_id": app.job_id,
                        "bpp_id": app.bpp_id,
                        "bpp_uri": app.bpp_uri,
                        "metadata": app.metadata,
                        "created_at": app.created_at,
                        "modified_at": app.modified_at


                    })
                })
                .collect();

            let body = json!({
                "user_id": user_id,
                "applications": apps_json,
            });

            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            tracing::error!("DB error fetching job applications: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
                .into_response()
        }
    }
}

pub async fn update_user_draft_application(
    Path(draft_id): Path<i32>,
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<DraftRequest>,
) -> Result<impl IntoResponse, Response> {
    let user_id = req
        .message
        .order
        .fulfillments
        .as_ref()
        .and_then(|fulfillments| fulfillments.get(0))
        .and_then(|f| f.customer.as_ref())
        .map(|c| c.person.id.clone())
        .unwrap_or_else(|| "".to_string());

    let job_id = req
        .message
        .order
        .items
        .get(0)
        .map(|item| item.id.clone())
        .unwrap_or_else(|| "".to_string());
    let bpp_id = req.context.bpp_id.clone();
    let bpp_uri = req.context.bpp_uri.clone();

    let metadata = match serde_json::to_value(&req.message) {
        Ok(val) => Some(val),
        Err(e) => {
            tracing::error!("Failed to convert message to JSON: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Invalid message format"})),
            )
                .into_response());
        }
    };

    let draft_data = UpdateJobDraft {
        user_id,
        job_id,
        bpp_id,
        bpp_uri,
        metadata,
        id: Some(draft_id),
    };

    match upsert_draft_application(&app_state.db_pool, draft_data).await {
        Ok(_) => {
            let response_body = json!({
                "message": "Draft application updated successfully"
            });
            Ok((StatusCode::OK, Json(response_body)))
        }
        Err(e) => {
            tracing::error!("DB error while saving draft: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to save draft application"})),
            )
                .into_response())
        }
    }
}

pub async fn delete_user_draft_application(
    Path(draft_id): Path<i32>,
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Response> {
    match delete_draft_application(&app_state.db_pool, draft_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(sqlx::Error::RowNotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Draft not found" })),
        )
            .into_response()),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("DB error: {:?}", e) })),
        )
            .into_response()),
    }
}
