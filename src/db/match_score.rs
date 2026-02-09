use serde_json::Value;
use sqlx::{query, query_as, query_scalar, Error, FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub hash: String,
    pub metadata: Option<Value>,
    pub beckn_structure: Option<Value>,
}

#[derive(Debug, FromRow)]
pub struct ProfileRow {
    pub id: Uuid,
    pub hash: String,
    pub metadata: Option<Value>,
    pub beckn_structure: Option<Value>,
}

#[derive(Debug, FromRow, Clone)]
pub struct JobLiteRow {
    pub id: Uuid,
    pub hash: String,
}

#[derive(Debug, FromRow, Clone)]
pub struct ProfileLiteRow {
    pub id: Uuid,
    pub hash: String,
}

#[derive(Debug, FromRow, Clone)]
pub struct StaleMatchRow {
    pub job_id: Uuid,
    pub profile_id: Uuid,
    pub job_hash: String,
    pub profile_hash: String,
    pub current_job_hash: String,
    pub current_profile_hash: String,
}

pub async fn fetch_new_jobs(pool: &PgPool) -> Result<Vec<JobLiteRow>, sqlx::Error> {
    query_as::<_, JobLiteRow>(
        r#"
        SELECT j.id, j.hash
        FROM jobs j
        LEFT JOIN job_profile_matches m
          ON m.job_id = j.id
        WHERE m.job_id IS NULL
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn fetch_new_profiles(pool: &PgPool) -> Result<Vec<ProfileLiteRow>, sqlx::Error> {
    query_as::<_, ProfileLiteRow>(
        r#"
        SELECT p.id, p.hash
        FROM profiles p
        LEFT JOIN job_profile_matches m
          ON m.profile_id = p.id
        WHERE m.profile_id IS NULL
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn fetch_stale_matches(pool: &PgPool) -> Result<Vec<StaleMatchRow>, sqlx::Error> {
    query_as::<_, StaleMatchRow>(
        r#"
        SELECT
          m.job_id,
          m.profile_id,
          m.job_hash,
          m.profile_hash,
          j.hash AS current_job_hash,
          p.hash AS current_profile_hash
        FROM job_profile_matches m
        JOIN jobs j ON j.id = m.job_id
        JOIN profiles p ON p.id = m.profile_id
        WHERE m.job_hash <> j.hash
           OR m.profile_hash <> p.hash
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn upsert_match_score(
    pool: &PgPool,
    job_id: Uuid,
    profile_id: Uuid,
    job_hash: &str,
    profile_hash: &str,
    match_score: i16,
    score_breakdown: Option<Value>,
) -> Result<(), sqlx::Error> {
    query!(
        r#"
        INSERT INTO job_profile_matches (
            job_id,
            profile_id,
            job_hash,
            profile_hash,
            match_score,
            score_breakdown,
            computed_at,
            updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, now(), now())
        ON CONFLICT (job_id, profile_id)
        DO UPDATE SET
            job_hash        = EXCLUDED.job_hash,
            profile_hash    = EXCLUDED.profile_hash,
            match_score     = EXCLUDED.match_score,
            score_breakdown = EXCLUDED.score_breakdown,
            updated_at      = now()
        "#,
        job_id,
        profile_id,
        job_hash,
        profile_hash,
        match_score,
        score_breakdown
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_job_by_id(pool: &PgPool, job_id: Uuid) -> Result<JobRow, sqlx::Error> {
    query_as::<_, JobRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure
        FROM jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
}

pub async fn fetch_profile_by_id(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<ProfileRow, sqlx::Error> {
    query_as::<_, ProfileRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure
        FROM profiles
        WHERE id = $1
        "#,
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await
}

pub async fn fetch_all_jobs(pool: &PgPool) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure
        FROM jobs
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn fetch_all_profiles(pool: &PgPool) -> Result<Vec<ProfileRow>, sqlx::Error> {
    query_as::<_, ProfileRow>(
        r#"
        SELECT
            id,
            hash,
            metadata,
            beckn_structure
        FROM profiles
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn fetch_jobs_with_matches(
    db_pool: &PgPool,
    profile_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Value, sqlx::Error> {
    match profile_id {
        Some(profile_id) => {
            let total: i64 = query_scalar(
                r#"
                SELECT COUNT(*)
                FROM job_profile_matches jpm
                JOIN profiles p ON p.id = jpm.profile_id
                WHERE p.profile_id = $1
                "#,
            )
            .bind(profile_id)
            .fetch_one(db_pool)
            .await?;

            let items: Vec<Value> = sqlx::query_scalar(
                r#"
                SELECT jsonb_build_object(
                    'job', to_jsonb(j.*),
                    'profile_id', p.profile_id,
                    'match_score', jpm.match_score
                )
                FROM job_profile_matches jpm
                JOIN jobs j ON j.id = jpm.job_id
                JOIN profiles p ON p.id = jpm.profile_id
                WHERE p.profile_id = $1
                ORDER BY jpm.match_score DESC, j.id ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(profile_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(db_pool)
            .await?;

            Ok(serde_json::json!({
                "total": total,
                "items": items
            }))
        }

        None => {
            let total: i64 = query_scalar(
                r#"
                SELECT COUNT(*)
                FROM jobs
                "#,
            )
            .fetch_one(db_pool)
            .await?;

            let items: Vec<Value> = query_scalar(
                r#"
                SELECT jsonb_build_object(
                    'job', to_jsonb(j.*),
                    'profile_id', NULL,
                    'match_score', NULL
                )
                FROM jobs j
                ORDER BY j.created_at DESC, j.id ASC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(db_pool)
            .await?;

            Ok(serde_json::json!({
                "total": total,
                "items": items
            }))
        }
    }
}
