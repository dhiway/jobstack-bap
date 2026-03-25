use crate::vector::faiss_service::FaissService;
use deadpool_redis::Pool;
use faiss::index::IndexImpl;
use faiss::{index_factory, read_index, write_index, MetricType};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

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
