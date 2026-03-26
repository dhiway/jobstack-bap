use crate::cron::start_cron_jobs;
use crate::workers::redis_event_worker::start as start_redis_worker;
use crate::{
    config::AppConfig,
    http::routes::create_routes,
    state::{AppState, SharedState},
};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use tracing::info;

use crate::vector::faiss_service::FaissService;
use crate::vector::index_store::load_faiss;
use deadpool_redis::{Config as RedisConfig, Pool, Runtime};
use tokio::sync::RwLock;

pub async fn start_http_server(
    config: AppConfig,
    shutdown_rx: watch::Receiver<()>,
) -> Result<
    JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let http_addr = format!("{}:{}", config.http.address, config.http.port);
    let listener = TcpListener::bind(http_addr.clone()).await?;
    info!("🚀 Starting BAP-WEBHOOK server on {:?}", http_addr);

    let shared_state = SharedState::default();

    let redis_cfg = RedisConfig::from_url(config.redis.url.as_str());
    let redis_pool: Pool = redis_cfg.create_pool(Some(Runtime::Tokio1))?;

    // Test Redis connection
    {
        let mut conn = redis_pool.get().await?;
        let pong: String = redis::cmd("PING").query_async(&mut conn).await?;
        info!("✅ Redis PING -> {}", pong);
    }

    // --- Postgres pool ---
    let db_pool = PgPool::connect(&config.db.url).await?;
    info!("✅ connected to db at {}", &config.db.url);

    let faiss = load_faiss(config.gcp.dimension, redis_pool.clone()).unwrap_or_else(|_| {
        info!("⚠️ No FAISS index found, creating new one");
        FaissService::new(config.gcp.dimension, redis_pool.clone())
    });

    let faiss = Arc::new(RwLock::new(faiss));

    let app_state = Arc::new(AppState {
        config: Arc::new(config.clone()),
        shared_state,
        redis_pool,
        db_pool,
        faiss,
    });

    let _scheduler = start_cron_jobs(app_state.clone()).await;

    let http_server = tokio::spawn(run_http_server(listener, shutdown_rx, app_state.clone()));

    {
        tokio::spawn(async move {
            start_redis_worker(app_state.clone()).await;
        });
    }

    Ok(http_server)
}

pub async fn run_http_server(
    listener: TcpListener,
    mut shutdown_rx: watch::Receiver<()>,
    app_state: Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = create_routes(app_state);

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
            info!("🚦 Gracefully shutting down all connections");
        })
        .await?;

    Ok(())
}
