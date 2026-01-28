use crate::models::search::Pagination;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{query, Error, PgPool};
use tracing::info;

pub struct PaginatedItems {
    pub items: Vec<Value>,
    pub total: i64,
}

pub struct NewProfile {
    pub profile_id: String,
    pub user_id: String,
    pub r#type: String,
    pub metadata: Option<Value>,
    pub beckn_structure: Option<Value>,
    pub hash: String,
    pub last_synced_at: DateTime<Utc>,
}
pub async fn store_profiles(db_pool: &PgPool, profiles: &[NewProfile]) -> Result<(), Error> {
    if profiles.is_empty() {
        return Ok(());
    }

    let profile_ids: Vec<&str> = profiles.iter().map(|p| p.profile_id.as_str()).collect();
    let user_ids: Vec<&str> = profiles.iter().map(|p| p.user_id.as_str()).collect();
    let types: Vec<&str> = profiles.iter().map(|p| p.r#type.as_str()).collect();
    let metadata: Vec<Option<&serde_json::Value>> =
        profiles.iter().map(|p| p.metadata.as_ref()).collect();
    let beckn_structure: Vec<Option<&serde_json::Value>> = profiles
        .iter()
        .map(|p| p.beckn_structure.as_ref())
        .collect();
    let hashes: Vec<&str> = profiles.iter().map(|p| p.hash.as_str()).collect();
    let last_synced_at: Vec<DateTime<Utc>> = profiles.iter().map(|p| p.last_synced_at).collect();

    sqlx::query(
        r#"
        INSERT INTO profiles (
            profile_id,
            user_id,
            type,
            metadata,
            beckn_structure,
            hash,
            last_synced_at
        )
        SELECT
            profile_id,
            user_id,
            type,
            metadata,
            beckn_structure,
            hash,
            last_synced_at
        FROM UNNEST(
            $1::text[],
            $2::text[],
            $3::text[],
            $4::jsonb[],
            $5::jsonb[],
            $6::text[],
            $7::timestamptz[]
        ) AS t(
            profile_id,
            user_id,
            type,
            metadata,
            beckn_structure,
            hash,
            last_synced_at
        )
        ON CONFLICT (profile_id) DO UPDATE
        SET
            user_id = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.user_id
                ELSE profiles.user_id
            END,
            type = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.type
                ELSE profiles.type
            END,
            metadata = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.metadata
                ELSE profiles.metadata
            END,
            beckn_structure = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.beckn_structure
                ELSE profiles.beckn_structure
            END,
            hash = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.hash
                ELSE profiles.hash
            END,
            updated_at = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN now()
                ELSE profiles.updated_at
            END,
            last_synced_at = EXCLUDED.last_synced_at
        "#,
    )
    .bind(&profile_ids)
    .bind(&user_ids)
    .bind(&types)
    .bind(&metadata)
    .bind(&beckn_structure)
    .bind(&hashes)
    .bind(&last_synced_at)
    .execute(db_pool)
    .await?;
    Ok(())
}

pub async fn delete_stale_profiles(
    pool: &PgPool,
    synced_at: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = query!(
        r#"
        DELETE FROM profiles
        WHERE last_synced_at IS NOT NULL
          AND last_synced_at < $1
        "#,
        synced_at
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn fetch_beckn_profile_items(
    db_pool: &PgPool,
    pagination: Pagination,
) -> Result<PaginatedItems, Error> {
    let page = pagination.page.unwrap_or(1).max(1);
    let limit = pagination.limit.unwrap_or(10).max(1);
    let offset = (page - 1) * limit;
    info!(
        "Fetching profiles - Page: {}, Limit: {}, Offset: {}",
        page, limit, offset
    );

    // ---- total count ----
    let total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        "#
    )
    .fetch_one(db_pool)
    .await?;

    // ---- paginated data ----
    let rows = sqlx::query!(
        r#"
        SELECT beckn_structure
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        ORDER BY updated_at DESC, profile_id DESC
        LIMIT $1
        OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(db_pool)
    .await?;

    let items = rows
        .into_iter()
        .filter_map(|r| r.beckn_structure)
        .collect::<Vec<_>>();

    Ok(PaginatedItems { items, total })
}
