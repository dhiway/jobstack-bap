use crate::middleware::api_key::api_key_auth;
use crate::services::events::handle_events;
use crate::state::AppState;

use axum::{middleware, routing::post, Router};
use std::sync::Arc;

pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/event", post(handle_events))
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            api_key_auth,
        ))
        .with_state(app_state)
}
