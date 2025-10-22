use crate::config::AppConfig;
use serde_json::Value;
use strsim::jaro_winkler;
use tracing::info;

fn weighted_push(parts: &mut Vec<String>, text: &str, weight: usize) {
    for _ in 0..weight {
        parts.push(text.to_string());
    }
}

pub fn profile_text_for_embedding(profile: &Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();
    for field in &config.metadata_match {
        if let Some(value) = profile.pointer(&field.profile_path) {
            if field.is_array {
                if let Some(arr) = value.as_array() {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            weighted_push(&mut parts, s, field.weight);
                        }
                    }
                }
            } else if let Some(s) = value.as_str() {
                weighted_push(&mut parts, s, field.weight);
            }
        }
    }
    parts.join(" ")
}

pub fn job_text_for_embedding(job: &Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();
    for field in &config.metadata_match {
        if let Some(value) = job.pointer(&field.job_path) {
            if field.is_array {
                if let Some(arr) = value.as_array() {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            weighted_push(&mut parts, s, field.weight);
                        }
                    }
                }
            } else if let Some(s) = value.as_str() {
                weighted_push(&mut parts, s, field.weight);
            }
        }
    }
    parts.join(" ")
}

pub fn cosine_similarity(vec_a: &[f32], vec_b: &[f32]) -> f32 {
    if vec_a.len() != vec_b.len() || vec_a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = vec_a.iter().zip(vec_b.iter()).map(|(a, b)| a * b).sum();
    let norm_a: f32 = vec_a.iter().map(|a| a * a).sum::<f32>().sqrt();
    let norm_b: f32 = vec_b.iter().map(|b| b * b).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Compute final match score combining embeddings + metadata

pub fn compute_match_score(
    profile_emb: &[f32],
    job_emb: &[f32],
    profile_meta: &Value,
    job_meta: &Value,
    config: &AppConfig,
) -> f32 {
    info!("🔍 Computing match score...");
    let mut score = cosine_similarity(profile_emb, job_emb);
    let base_score = score;
    info!("🧮 Base cosine similarity score: {:.4}", base_score);

    let mut mismatches = 0;

    for field in &config.metadata_match {
        if let (Some(profile_val), Some(job_val)) = (
            profile_meta.pointer(&field.profile_path),
            job_meta.pointer(&field.job_path),
        ) {
            let similarity = if field.compare_mode == "string" {
                jaro_winkler(
                    profile_val.as_str().unwrap_or_default(),
                    job_val.as_str().unwrap_or_default(),
                )
            } else {
                // future support for numeric comparison
                1.0
            };

            if similarity < 0.8 {
                score *= field.penalty;
                mismatches += 1;
                info!(
                    "⚠️ {} mismatch ('{}' != '{}', similarity {:.2}) → penalized score to {:.4}",
                    field.name, profile_val, job_val, similarity, score
                );
            }
        } else {
            score *= field.penalty;
            mismatches += 1;
            info!(
                "⚠️ {} missing → applied penalty {:.4}, score now {:.4}",
                field.name, field.penalty, score
            );
        }
    }

    // Additional penalties for multiple mismatches
    match mismatches {
        2 => score *= 0.85,
        3..=usize::MAX => score *= 0.7,
        _ => {}
    }

    if score.is_nan() {
        score = 0.0;
        info!("🚫 NaN detected — setting score to 0.0");
    }
    score = score.clamp(0.0, 1.0);
    info!(
        "✅ Final match score: {:.4} (base {:.4})",
        score, base_score
    );

    score
}
