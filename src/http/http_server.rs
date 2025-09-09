use crate::cron::start_cron_jobs;
use crate::{
    config::AppConfig,
    http::routes::create_routes,
    state::{AppState, SharedState},
};
use redis::Client;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    sync::{watch, Mutex},
    task::JoinHandle,
};
use tracing::info;

pub async fn start_http_server(
    config: AppConfig,
    shutdown_rx: watch::Receiver<()>,
) -> Result<
    JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let http_addr = format!("{}:{}", config.http.address, config.http.port);
    let listener = tokio::net::TcpListener::bind(http_addr.clone()).await?;
    info!("🚀 Starting BAP-WEBHOOK server on {:?}", http_addr);

    let shared_state = SharedState::default();

    let redis_client = Client::open(config.redis.url.as_str())?;
    let redis_conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| {
            tracing::error!("❌ Redis connection failed: {}", e);
            e
        })?;

    // Test Redis connection
    {
        let mut test_conn = redis_client.get_multiplexed_async_connection().await?;
        let pong: String = redis::cmd("PING").query_async(&mut test_conn).await?;
        info!("✅ Redis PING -> {}", pong);
    }

    let db_pool = PgPool::connect(&config.db.url).await?;
    info!("✅ connected to db at {}", &config.db.url);
    let app_state = AppState {
        config: Arc::new(config.clone()),
        shared_state,
        redis_conn: Arc::new(Mutex::new(redis_conn)),
        db_pool,
    };
    let _scheduler = start_cron_jobs(app_state.clone()).await;

    let http_server = tokio::spawn(run_http_server(listener, shutdown_rx, app_state));

    Ok(http_server)
}

pub async fn run_http_server(
    listener: TcpListener,
    mut shutdown_rx: watch::Receiver<()>,
    app_state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = create_routes(app_state.clone());

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
            tracing::info!("🚦 Gracefully shutting down all connections, ");
        })
        .await?;

    Ok(())
}
