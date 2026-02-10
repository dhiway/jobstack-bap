use anyhow::Result;
use async_trait::async_trait;
use hex;
use redis::AsyncCommands;
use reqwest;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{error, info};

use crate::state::AppState;

#[async_trait]
pub trait EmbeddingService {
    async fn get_embedding(
        &self,
        text: &str,
        conn: &mut redis::aio::MultiplexedConnection,
        app_state: &AppState,
    ) -> Result<Vec<f32>>;
}

pub struct GcpEmbeddingService;

#[async_trait]
impl EmbeddingService for GcpEmbeddingService {
    async fn get_embedding(
        &self,
        text: &str,
        conn: &mut redis::aio::MultiplexedConnection,
        app_state: &AppState,
    ) -> Result<Vec<f32>> {
        if text.trim().is_empty() {
            info!("⚠️ Skipping embedding generation: input text is empty");
            return Ok(vec![]);
        }
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash = hex::encode(hasher.finalize());

        let cache_key = format!("embedding:{}:{}", app_state.config.gcp.model, hash);

        info!(
            "🔑 Embedding request | chars={} | cache_key={}",
            text.len(),
            cache_key
        );

        match conn.get::<_, Option<String>>(&cache_key).await {
            Ok(Some(cached)) => {
                if let Ok(vec) = serde_json::from_str::<Vec<f32>>(&cached) {
                    info!("✅ Embedding cache hit");
                    return Ok(vec);
                } else {
                    info!("⚠️ Failed to deserialize cached embedding, refetching");
                }
            }
            Ok(None) => info!("❌ Embedding cache miss"),
            Err(e) => error!("❌ Redis get error: {:?}", e),
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent",
            app_state.config.gcp.model
        );

        let body = serde_json::json!({
            "model": format!("models/{}", app_state.config.gcp.model),
            "content": {
                "parts": [
                    { "text": text }
                ]
            }
        });

        info!("🚀 Fetching embedding from GCP");

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("x-goog-api-key", &app_state.config.gcp.auth_token)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        info!("📡 GCP embedding response status: {}", status);

        let json_resp: Value = resp.json().await?;

        if !status.is_success() {
            error!("❌ GCP embedding API error response: {:?}", json_resp);
            return Err(anyhow::anyhow!(
                "GCP embedding API failed with status {}",
                status
            ));
        }

        let embedding_values = json_resp
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing embedding.values in GCP response"))?;

        let embedding: Vec<f32> = embedding_values
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        if let Err(e) = conn
            .set::<_, _, ()>(&cache_key, serde_json::to_string(&embedding)?)
            .await
        {
            error!("❌ Failed to cache embedding in Redis: {:?}", e);
        } else {
            info!("💾 Cached embedding | dims={}", embedding.len());
        }

        Ok(embedding)
    }
}
