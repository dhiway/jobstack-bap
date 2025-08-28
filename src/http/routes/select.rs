use crate::services::select::handle_select;
use crate::state::AppState;
use axum::{routing::post, Router};

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/select", post(handle_select))
        .with_state(app_state)
}
