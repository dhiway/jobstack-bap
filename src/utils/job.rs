use crate::db::job::{batch_update_job_embeddings, fetch_jobs_pending_embedding};
use crate::services::empeding::{EmbeddingService, GcpEmbeddingService};
use crate::state::AppState;
use crate::utils::empeding::job_text_for_embedding;
use std::sync::Arc;
use tracing::{error, info};

pub async fn update_embeddings_for_bpp(
    app_state: &Arc<AppState>,
    bpp_id: &str,
) -> Result<(), anyhow::Error> {
    let embedding_service = GcpEmbeddingService;
    let mut redis = app_state.redis_pool.get().await.map_err(|e| {
        error!("Redis connection failed: {}", e);
        e
    })?;

    let jobs = fetch_jobs_pending_embedding(&app_state.db_pool, bpp_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch jobs for embedding: {}", e);
            e
        })?;

    if jobs.is_empty() {
        info!("No jobs found for embedding (bpp_id={})", bpp_id);
        return Ok(());
    }

    info!(
        "Generating embeddings for {} jobs (bpp_id={})",
        jobs.len(),
        bpp_id
    );

    let batch_size = 20;

    for chunk in jobs.chunks(batch_size) {
        let mut updates: Vec<(uuid::Uuid, Vec<f32>)> = Vec::new();

        for job in chunk {
            let beckn = match job.beckn_structure.as_ref() {
                Some(b) => b,
                None => continue,
            };

            let text = job_text_for_embedding(beckn, &app_state.config);
            info!(
                "Generating embedding for job_id={} (text chars={})",
                job.job_id,
                text.len()
            );

            match embedding_service
                .get_embedding(&text, &mut redis, app_state)
                .await
            {
                Ok(embedding) if !embedding.is_empty() => {
                    updates.push((job.id, embedding));
                }
                Err(e) => {
                    error!("Embedding failed for job_id={}: {}", job.job_id, e);
                }
                _ => {
                    error!("Empty embedding for job_id={}", job.job_id);
                }
            }
        }

        if !updates.is_empty() {
            if let Err(e) = batch_update_job_embeddings(&app_state.db_pool, &updates).await {
                error!("Batch embedding update failed: {}", e);
            }
        }
    }

    info!("Embedding generation completed (bpp_id={})", bpp_id);

    Ok(())
}
