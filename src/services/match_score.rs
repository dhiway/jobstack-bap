use crate::utils::empeding::{
    compute_empeding_match_score, job_text_for_embedding, profile_text_for_embedding,
};
use crate::{
    db::match_score::{JobRow, ProfileRow},
    state::AppState,
};
use serde_json::Value;
use serde_json::{json, Value as JsonValue};

use crate::services::empeding::{EmbeddingService, GcpEmbeddingService};
pub async fn compute_match_score(
    app_state: &AppState,
    job: &JobRow,
    profile: &ProfileRow,
) -> (i16, Option<Value>) {
    let source = &app_state.config.cron.compute_match_scores.source;

    match source.as_str() {
        "empeding" => compute_match_score_empeding(app_state, job, profile).await,
        _ => compute_match_score_empeding(app_state, job, profile).await,
    }
}

pub async fn compute_match_score_empeding(
    app_state: &AppState,
    job: &JobRow,
    profile: &ProfileRow,
) -> (i16, Option<JsonValue>) {
    let embedding_service = GcpEmbeddingService;

    let result: Option<(i16, Option<JsonValue>)> = async {
        let mut conn = app_state.redis_pool.get().await.ok()?;

        let metadata = profile.metadata.as_ref()?;
        let profile_meta = json!({ "metadata": metadata });

        let profile_text = profile_text_for_embedding(&profile_meta, &app_state.config);
        let profile_emb = embedding_service
            .get_embedding(&profile_text, &mut conn, app_state)
            .await
            .ok()?;

        let beckn_structure = job.beckn_structure.as_ref()?;
        let job_text = job_text_for_embedding(beckn_structure, &app_state.config);
        let job_emb = embedding_service
            .get_embedding(&job_text, &mut conn, app_state)
            .await
            .ok()?;

        let profile_norm = profile_emb.iter().map(|x| x * x).sum::<f32>().sqrt();

        let job_norm = job_emb.iter().map(|x| x * x).sum::<f32>().sqrt();

        let mut string_sim_cache = std::collections::HashMap::new();

        let score = compute_empeding_match_score(
            &profile_emb,
            profile_norm,
            &job_emb,
            job_norm,
            &profile_meta,
            beckn_structure,
            &app_state.config,
            &mut string_sim_cache,
        );

        let score_i16 = (score * 10.0).round() as i16;

        Some((score_i16, Some(json!({ "cosine_score": score }))))
    }
    .await;

    result.unwrap_or((0, None))
}
