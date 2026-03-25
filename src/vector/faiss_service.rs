use deadpool_redis::redis::cmd;
use deadpool_redis::Pool;
use faiss::index::IndexImpl;
use faiss::selector::IdSelector;
use faiss::{index_factory, Index, MetricType};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
#[derive(Clone)]
pub struct FaissService {
    pub index: Arc<Mutex<IndexImpl>>,
    pub redis_pool: Pool,
}

impl FaissService {
    pub fn new(dimension: u32, redis_pool: Pool) -> Self {
        let index: IndexImpl = index_factory(dimension, "IDMap,Flat", MetricType::InnerProduct)
            .expect("FAISS init failed");

        Self {
            index: Arc::new(Mutex::new(index)),
            redis_pool,
        }
    }

    async fn get_or_create_faiss_id(
        &self,
        job_id: Uuid,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;

        let job_key = format!("faiss:job_to_id:{}", job_id);

        let existing_id: Option<i64> = cmd("GET").arg(&job_key).query_async(&mut conn).await?;

        if let Some(id) = existing_id {
            return Ok(id);
        }

        let new_id: i64 = cmd("INCR")
            .arg("faiss:next_id")
            .query_async(&mut conn)
            .await?;

        let reverse_key = format!("faiss:id_to_job:{}", new_id);

        let _: () = cmd("SET")
            .arg(&job_key)
            .arg(new_id)
            .query_async(&mut conn)
            .await?;

        let _: () = cmd("SET")
            .arg(&reverse_key)
            .arg(job_id.to_string())
            .query_async(&mut conn)
            .await?;

        Ok(new_id)
    }

    async fn get_job_id_from_faiss_id(
        &self,
        faiss_id: i64,
    ) -> Result<Option<Uuid>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;

        let reverse_key = format!("faiss:id_to_job:{}", faiss_id);

        let job_id_str: Option<String> =
            cmd("GET").arg(&reverse_key).query_async(&mut conn).await?;

        match job_id_str {
            Some(s) => Ok(Some(Uuid::parse_str(&s)?)),
            None => Ok(None),
        }
    }

    pub async fn add(
        &self,
        job_id: Uuid,
        mut embedding: Vec<f32>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        normalize(&mut embedding);

        let faiss_id = self.get_or_create_faiss_id(job_id).await?;

        let mut index = self.index.lock().await;
        index.add_with_ids(&embedding, &[faiss_id.into()])?;

        Ok(())
    }

    pub async fn search(
        &self,
        mut query: Vec<f32>,
        k: usize,
    ) -> Result<Vec<(Uuid, f32)>, Box<dyn std::error::Error + Send + Sync>> {
        normalize(&mut query);

        let mut index = self.index.lock().await;
        let result = index.search(&query, k)?;
        drop(index);

        let mut output = Vec::new();

        for (idx, score) in result.labels.iter().zip(result.distances.iter()) {
            let raw_id = match idx.get() {
                Some(v) => v as i64,
                None => continue,
            };

            if let Some(job_id) = self.get_job_id_from_faiss_id(raw_id).await? {
                output.push((job_id, *score));
            }
        }

        Ok(output)
    }

    async fn get_faiss_id(
        &self,
        job_id: Uuid,
    ) -> Result<Option<i64>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;
        let job_key = format!("faiss:job_to_id:{}", job_id);

        let faiss_id: Option<i64> = cmd("GET").arg(&job_key).query_async(&mut conn).await?;
        Ok(faiss_id)
    }

    async fn delete_faiss_mappings(
        &self,
        job_id: Uuid,
        faiss_id: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.redis_pool.get().await?;

        let job_key = format!("faiss:job_to_id:{}", job_id);
        let reverse_key = format!("faiss:id_to_job:{}", faiss_id);

        let _: () = cmd("DEL")
            .arg(&job_key)
            .arg(&reverse_key)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn upsert(
        &self,
        job_id: Uuid,
        mut embedding: Vec<f32>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        normalize(&mut embedding);

        let faiss_id = match self.get_faiss_id(job_id).await? {
            Some(id) => id,
            None => self.get_or_create_faiss_id(job_id).await?,
        };

        let mut index = self.index.lock().await;

        let selector = IdSelector::batch(&[faiss_id.into()])?;
        let _ = index.remove_ids(&selector);

        index.add_with_ids(&embedding, &[faiss_id.into()])?;

        Ok(())
    }

    pub async fn remove(
        &self,
        job_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let faiss_id = match self.get_faiss_id(job_id).await? {
            Some(id) => id,
            None => return Ok(()),
        };

        let mut index = self.index.lock().await;

        let selector = IdSelector::batch(&[faiss_id.into()])?;
        let _ = index.remove_ids(&selector);

        drop(index);

        self.delete_faiss_mappings(job_id, faiss_id).await?;

        Ok(())
    }
}

fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}
