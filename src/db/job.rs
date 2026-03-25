use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{query, query_as, Error, FromRow, PgPool};
use uuid::Uuid;
#[derive(Debug, FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub hash: String,
    pub metadata: Option<Value>,
    pub beckn_structure: Option<Value>,
    pub embedding: Option<Vec<f32>>,
    pub job_id: String,
    pub bpp_id: String,
}

#[derive(sqlx::FromRow, Debug)]
pub struct JobLookup {
    pub job_id: String,
    pub provider_id: Option<String>,
    pub bpp_id: Option<String>,
    pub bpp_uri: Option<String>,
}

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
            embedding = CASE
                WHEN jobs.hash IS DISTINCT FROM EXCLUDED.hash
                THEN NULL
                ELSE jobs.embedding
            END,
            transaction_id = EXCLUDED.transaction_id,
            bpp_id = EXCLUDED.bpp_id,
            bpp_uri = EXCLUDED.bpp_uri,
            last_synced_at = EXCLUDED.last_synced_at,
            is_active = true,
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

pub async fn deactivate_stale_jobs(
    db_pool: &PgPool,
    bpp_id: &str,
    txn_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = query(
        r#"
        UPDATE jobs
        SET is_active = false,
            updated_at = now()
        WHERE bpp_id = $1
          AND transaction_id <> $2
          AND is_active = true
        "#,
    )
    .bind(bpp_id)
    .bind(txn_id)
    .execute(db_pool)
    .await?;

    Ok(result.rows_affected())
}
pub async fn fetch_job_by_id(pool: &PgPool, job_id: Uuid) -> Result<JobRow, sqlx::Error> {
    query_as::<_, JobRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure,
            job_id,
            bpp_id,
            embedding
        FROM jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
}

pub async fn fetch_job_by_job_id(pool: &PgPool, job_id: &str) -> Result<JobLookup, sqlx::Error> {
    query_as::<_, JobLookup>(
        r#"
        SELECT
            job_id,
            provider_id,
            bpp_id,
            bpp_uri
        FROM jobs
        WHERE job_id = $1
          AND is_active = true
        "#,
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
}

pub async fn fetch_jobs_pending_embedding(
    db_pool: &PgPool,
    bpp_id: &str,
) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT *
        FROM jobs
        WHERE embedding IS NULL
        AND bpp_id = $1
        AND is_active = true
        "#,
    )
    .bind(bpp_id)
    .fetch_all(db_pool)
    .await
}
pub async fn batch_update_job_embeddings(
    db_pool: &PgPool,
    updates: &[(uuid::Uuid, Vec<f32>)],
) -> Result<(), sqlx::Error> {
    for (id, embedding) in updates {
        sqlx::query(
            r#"
            UPDATE jobs
            SET embedding = $1
            WHERE id = $2
            "#,
        )
        .bind(embedding)
        .bind(id)
        .execute(db_pool)
        .await?;
    }

    Ok(())
}

pub async fn fetch_jobs_by_ids(
    pool: &PgPool,
    job_ids: &[Uuid],
) -> Result<Vec<JobRow>, sqlx::Error> {
    if job_ids.is_empty() {
        return Ok(vec![]);
    }

    let jobs = query_as::<_, JobRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure,
            job_id,
            bpp_id,
            embedding
        FROM jobs
        WHERE id = ANY($1)
        "#,
    )
    .bind(job_ids)
    .fetch_all(pool)
    .await?;

    Ok(jobs)
}
