use crate::db::job::stream_active_jobs_with_embeddings;
use crate::state::AppState;
use crate::utils::redis::delete_keys_by_pattern;
use crate::vector::faiss_service::FaissService;
use deadpool_redis::{redis::cmd, Pool};
use faiss::index::IndexImpl;
use faiss::{index_factory, read_index, write_index, MetricType};
use futures::TryStreamExt;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

const INDEX_FILE: &str = "faiss.index";

pub async fn save_faiss(
    service: &FaissService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let index = service.index.lock().await;
    write_index(&*index, INDEX_FILE)?;
    Ok(())
}

pub fn load_faiss(
    dimension: u32,
    redis_pool: Pool,
) -> Result<FaissService, Box<dyn std::error::Error + Send + Sync>> {
    let index: IndexImpl = if Path::new(INDEX_FILE).exists() {
        read_index(INDEX_FILE)?
    } else {
        index_factory(dimension, "IDMap,Flat", MetricType::InnerProduct)?
    };

    Ok(FaissService {
        index: Arc::new(Mutex::new(index)),
        redis_pool,
    })
}

pub async fn rebuild_faiss_from_db(
    app_state: &Arc<AppState>,
    dimension: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting full FAISS rebuild from DB");

    {
        let mut conn = app_state.redis_pool.get().await?;

        delete_keys_by_pattern(&mut conn, "faiss:*").await?;

        let _: () = cmd("DEL")
            .arg("faiss:next_id")
            .query_async(&mut conn)
            .await?;
    }

    let fresh_index: IndexImpl = index_factory(dimension, "IDMap,Flat", MetricType::InnerProduct)?;

    {
        let faiss = app_state.faiss.read().await;
        let mut index = faiss.index.lock().await;
        *index = fresh_index;
    }

    let mut rows = stream_active_jobs_with_embeddings(&app_state.db_pool);

    {
        let faiss = app_state.faiss.read().await;
        let mut count: u64 = 0;

        while let Some(job) = rows.try_next().await? {
            if let Err(e) = faiss.upsert(job.id, job.embedding).await {
                error!("Failed to reindex job_id={}: {}", job.id, e);
                continue;
            }
            count += 1;
        }

        info!("Rebuilt FAISS with {} jobs", count);

        save_faiss(&faiss).await?;
    }

    info!("FAISS rebuild completed successfully");
    Ok(())
}
