use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{query, Error, PgPool};

pub struct NewJob {
    pub job_id: String,
    pub provider_id: String,
    pub transaction_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
    pub metadata: Option<Value>,
    pub beckn_structure: Option<Value>,
    pub hash: String,
    pub last_synced_at: Option<DateTime<Utc>>,
}

pub async fn store_jobs(db_pool: &PgPool, jobs: &[NewJob]) -> Result<(), Error> {
    if jobs.is_empty() {
        return Ok(());
    }

    let job_ids: Vec<&str> = jobs.iter().map(|j| j.job_id.as_str()).collect();
    let provider_ids: Vec<&str> = jobs.iter().map(|j| j.provider_id.as_str()).collect();

    let beckn_structures: Vec<Option<&Value>> =
        jobs.iter().map(|j| j.beckn_structure.as_ref()).collect();

    let metadata: Vec<Option<&Value>> = jobs.iter().map(|j| j.metadata.as_ref()).collect();

    let hashes: Vec<&str> = jobs.iter().map(|j| j.hash.as_str()).collect();

    let last_synced_at: Vec<Option<DateTime<Utc>>> =
        jobs.iter().map(|j| j.last_synced_at).collect();

    let transaction_ids: Vec<&str> = jobs.iter().map(|j| j.transaction_id.as_str()).collect();

    let bpp_ids: Vec<&str> = jobs.iter().map(|j| j.bpp_id.as_str()).collect();

    let bpp_uris: Vec<&str> = jobs.iter().map(|j| j.bpp_uri.as_str()).collect();

    query(
        r#"
        INSERT INTO jobs (
            job_id,
            provider_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        )
        SELECT
            job_id,
            provider_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        FROM UNNEST(
            $1::text[],
            $2::text[],
            $3::jsonb[],
            $4::jsonb[],
            $5::text[],
            $6::timestamptz[],
            $7::text[],
            $8::text[],
            $9::text[]
        ) AS t(
            job_id,
            provider_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        )
        ON CONFLICT (job_id, provider_id) DO UPDATE
        SET
            beckn_structure = CASE
                WHEN jobs.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.beckn_structure
                ELSE jobs.beckn_structure
            END,
            metadata = CASE
                WHEN jobs.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.metadata
                ELSE jobs.metadata
            END,
            hash = CASE
                WHEN jobs.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.hash
                ELSE jobs.hash
            END,
            transaction_id = EXCLUDED.transaction_id,
            bpp_id = EXCLUDED.bpp_id,
            bpp_uri = EXCLUDED.bpp_uri,
            last_synced_at = EXCLUDED.last_synced_at,
            updated_at = now()
        "#,
    )
    .bind(&job_ids)
    .bind(&provider_ids)
    .bind(&beckn_structures)
    .bind(&metadata)
    .bind(&hashes)
    .bind(&last_synced_at)
    .bind(&transaction_ids)
    .bind(&bpp_ids)
    .bind(&bpp_uris)
    .execute(db_pool)
    .await?;

    Ok(())
}

pub async fn delete_stale_jobs(
    db_pool: &PgPool,
    bpp_id: &str,
    txn_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = query(
        r#"
        DELETE FROM jobs
        WHERE bpp_id = $1
          AND transaction_id <> $2
        "#,
    )
    .bind(bpp_id)
    .bind(txn_id)
    .execute(db_pool)
    .await?;

    Ok(result.rows_affected())
}
