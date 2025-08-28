use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{query, Error, FromRow, PgPool, Postgres, QueryBuilder};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct NewJobApplication {
    pub user_id: String,
    pub job_id: String,
    pub order_id: String,
    pub transaction_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
    pub status: Option<String>,
    pub metadata: Option<Value>,
}

pub async fn store_job_applications(
    db_pool: &PgPool,
    data: NewJobApplication,
) -> Result<(), Error> {
    query(
        r#"
        INSERT INTO job_applications
            (user_id, job_id, order_id, transaction_id, bpp_id, bpp_uri, status, metadata)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(&data.user_id)
    .bind(&data.job_id)
    .bind(&data.order_id)
    .bind(&data.transaction_id)
    .bind(&data.bpp_id)
    .bind(&data.bpp_uri)
    .bind(data.status.unwrap_or_else(|| "pending".to_string()))
    .bind(data.metadata)
    .execute(db_pool)
    .await?;

    Ok(())
}

pub async fn get_job_applications(
    db_pool: &PgPool,
    user_id: Option<&str>,
    job_id: Option<&str>,
) -> Result<Vec<NewJobApplication>, Error> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT user_id, job_id, order_id, transaction_id, bpp_id, bpp_uri, status, metadata
        FROM job_applications
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

    let query = query_builder.build_query_as::<NewJobApplication>();

    let results = query.fetch_all(db_pool).await?;
    Ok(results)
}
