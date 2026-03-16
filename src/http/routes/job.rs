use crate::services::job_apply::{handle_job_applications, handle_job_apply};
use crate::services::job_draft::{
    create_user_draft_application, delete_user_draft_application, get_user_draft_applications,
    update_user_draft_application,
};
use crate::state::AppState;
use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/apply", post(handle_job_apply))
        .route("/v1/job-applications", get(handle_job_applications))
        .route(
            "/v1/job-applications/drafts",
            post(create_user_draft_application),
        )
        .route(
            "/v1/job-applications/drafts",
            get(get_user_draft_applications),
        )
        .route(
            "/v1/job-applications/drafts/{id}",
            patch(update_user_draft_application),
        )
        .route(
            "/v1/job-applications/drafts/{id}",
            delete(delete_user_draft_application),
        )
        .with_state(app_state)
}
