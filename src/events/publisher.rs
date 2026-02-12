use crate::models::events::AppEvent;

use deadpool_redis::Connection;

pub async fn publish_event(
    conn: &mut Connection,
    stream: &str,
    event: &AppEvent,
) -> redis::RedisResult<()> {
    let event_json = serde_json::to_string(event).unwrap();

    redis::cmd("XADD")
        .arg(stream)
        .arg("*")
        .arg("event")
        .arg(event_json)
        .query_async(conn)
        .await
}
