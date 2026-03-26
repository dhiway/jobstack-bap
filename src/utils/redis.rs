use deadpool_redis::redis::cmd;

pub async fn delete_keys_by_pattern(
    conn: &mut deadpool_redis::Connection,
    pattern: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut cursor: u64 = 0;

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await?;

        if !keys.is_empty() {
            let _: () = cmd("DEL").arg(keys).query_async(conn).await?;
        }

        if next_cursor == 0 {
            break;
        }

        cursor = next_cursor;
    }

    Ok(())
}
