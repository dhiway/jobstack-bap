use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

pub type OnSearchResponse = serde_json::Value;

use crate::config::AppConfig;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub shared_state: SharedState,
    pub redis_conn: Arc<Mutex<MultiplexedConnection>>,
    pub db_pool: PgPool,
}

#[derive(Clone, Default)]
pub struct SharedState {
    pub pending_searches: Arc<DashMap<String, oneshot::Sender<OnSearchResponse>>>,
}
