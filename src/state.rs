use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

pub type OnSearchResponse = serde_json::Value;

use crate::config::AppConfig;
use crate::vector::faiss_service::FaissService;
use deadpool_redis::Pool;
use sqlx::PgPool;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub shared_state: SharedState,
    pub redis_pool: Pool,
    pub db_pool: PgPool,
    pub faiss: Arc<RwLock<FaissService>>,
}

#[derive(Clone, Default)]
pub struct SharedState {
    pub pending_searches: Arc<DashMap<String, oneshot::Sender<OnSearchResponse>>>,
}
