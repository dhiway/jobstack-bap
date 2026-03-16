use deadpool_redis::Connection;
use redis::{cmd, streams::StreamReadReply, RedisResult};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::events::consumer::ensure_consumer_group;
use crate::events::handler::handle_event;
use crate::models::events::AppEvent;
use crate::state::AppState;

pub const STREAM_NAME: &str = "app_events";
const GROUP_NAME: &str = "bap_group";
const CONSUMER_NAME: &str = "worker_1";

pub async fn start(state: Arc<AppState>) {
    info!("Starting Redis Event Worker...");

    loop {
        if let Err(e) = run_worker(state.clone()).await {
            error!("Redis worker crashed: {:?}", e);
            sleep(Duration::from_secs(3)).await;
        }
    }
}

async fn run_worker(state: Arc<AppState>) -> RedisResult<()> {
    let mut conn: Connection = state.redis_pool.get().await.map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::IoError,
            "deadpool get failed",
            e.to_string(),
        ))
    })?;

    ensure_consumer_group(&mut conn, STREAM_NAME, GROUP_NAME).await?;

    info!("Redis worker listening on stream '{}'", STREAM_NAME);

    loop {
        process_stream(&state).await?;
    }
}

async fn process_stream(state: &Arc<AppState>) -> RedisResult<()> {
    let mut conn: Connection = state.redis_pool.get().await.map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::IoError,
            "deadpool get failed",
            e.to_string(),
        ))
    })?;

    let reply: StreamReadReply = cmd("XREADGROUP")
        .arg("GROUP")
        .arg(GROUP_NAME)
        .arg(CONSUMER_NAME)
        .arg("COUNT")
        .arg(10)
        .arg("BLOCK")
        .arg(5000)
        .arg("STREAMS")
        .arg(STREAM_NAME)
        .arg(">")
        .query_async(&mut conn)
        .await?;

    for stream in reply.keys {
        for message in stream.ids {
            if let Some(event) = parse_event(&message.map) {
                if let Err(e) = handle_event(state, event).await {
                    error!("Failed to handle event: {:?}", e);
                } else {
                    ack_message(&mut conn, &message.id).await?;
                }
            }
        }
    }

    Ok(())
}

fn parse_event(map: &std::collections::HashMap<String, redis::Value>) -> Option<AppEvent> {
    let value = map.get("event")?;
    let json_str: String = redis::from_redis_value(value).ok()?;
    serde_json::from_str(&json_str).ok()
}

async fn ack_message(conn: &mut Connection, message_id: &str) -> RedisResult<()> {
    cmd("XACK")
        .arg(STREAM_NAME)
        .arg(GROUP_NAME)
        .arg(message_id)
        .query_async::<()>(conn)
        .await
}
