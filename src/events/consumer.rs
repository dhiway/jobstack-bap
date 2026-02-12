use redis::aio::MultiplexedConnection;
use redis::{RedisError, RedisResult};

pub async fn ensure_consumer_group(
    conn: &mut MultiplexedConnection,
    stream: &str,
    group: &str,
) -> RedisResult<()> {
    let result: RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(group)
        .arg("$")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Consumer group '{}' created on stream '{}'", group, stream);
            Ok(())
        }
        Err(e) => {
            if is_busy_group_error(&e) {
                tracing::info!("Consumer group '{}' already exists", group);
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

fn is_busy_group_error(err: &RedisError) -> bool {
    err.to_string().contains("BUSYGROUP")
}
