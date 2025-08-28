use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{query, Error, FromRow, PgPool, Postgres, QueryBuilder};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DraftJobApplications {
    pub id: i32,
    pub user_id: String,
    pub job_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobDraft {
    pub user_id: String,
    pub job_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
    pub metadata: Option<Value>,
    pub id: Option<i32>,
}
pub type UpdateJobDraft = JobDraft;
pub type CreateJobDraft = JobDraft;

pub async fn get_draft_applications(
    db_pool: &PgPool,
    user_id: Option<&str>,
    job_id: Option<&str>,
) -> Result<Vec<DraftJobApplications>, Error> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT id, user_id, job_id, bpp_id, bpp_uri, metadata, created_at, modified_at
        FROM draft_applications
        "#,
    );

    let mut has_condition = false;

    if user_id.is_some() || job_id.is_some() {
        query_builder.push(" WHERE ");
    }

    if let Some(uid) = user_id {
        query_builder.push("user_id = ");
        query_builder.push_bind(uid);
        has_condition = true;
    }

    if let Some(jid) = job_id {
        if has_condition {
            query_builder.push(" AND ");
        }
        query_builder.push("job_id = ");
        query_builder.push_bind(jid);
    }

    if job_id.is_some() {
        query_builder.push(" LIMIT 1");
    }

    let query = query_builder.build_query_as::<DraftJobApplications>();

    let results = query.fetch_all(db_pool).await?;
    Ok(results)
}

pub async fn upsert_draft_application(db_pool: &PgPool, data: JobDraft) -> Result<(), Error> {
    if let Some(id) = &data.id {
        // Do UPDATE by ID
        query(
            r#"
            UPDATE draft_applications
            SET
                bpp_uri = $1,
                metadata = $2,
                modified_at = now()
            WHERE id = $3
            "#,
        )
        .bind(&data.bpp_uri)
        .bind(data.metadata)
        .bind(id)
        .execute(db_pool)
        .await?;
    } else {
        // INSERT or UPDATE on conflict
        query(
            r#"
            INSERT INTO draft_applications
                (user_id, job_id, bpp_id, bpp_uri, metadata)
            VALUES
                ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, job_id, bpp_id)
            DO UPDATE SET
                bpp_uri = EXCLUDED.bpp_uri,
                metadata = EXCLUDED.metadata,
                modified_at = now()
            "#,
        )
        .bind(&data.user_id)
        .bind(&data.job_id)
        .bind(&data.bpp_id)
        .bind(&data.bpp_uri)
        .bind(data.metadata)
        .execute(db_pool)
        .await?;
    }

    Ok(())
}

pub async fn delete_draft_application(db_pool: &PgPool, id: i32) -> Result<(), Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM draft_applications
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(db_pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}
