use crate::config::AppConfig;
use serde_json::Value;
use strsim::jaro_winkler;
use tracing::info;

fn weighted_push(parts: &mut Vec<String>, text: &str, weight: usize) {
    for _ in 0..weight {
        parts.push(text.to_string());
    }
}

pub fn job_text_for_embedding(job: &serde_json::Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();

    // Use the dynamic embedding_weights from config
    for field in &config.embedding_weights.job {
        if let Some(value) = job.pointer(&field.path) {
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

pub fn profile_text_for_embedding(profile: &serde_json::Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();

    for field in &config.embedding_weights.profile {
        if let Some(value) = profile.pointer(&field.path) {
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
) -> f32 {
    info!("🔍 Computing match score...");

    // --- Base cosine similarity ---
    let mut score = cosine_similarity(profile_emb, job_emb);
    let base_score = score;
    info!("🧮 Base cosine similarity score: {:.4}", base_score);

    let mut mismatches = 0;

    // --- ROLE MATCH ---
    if let Some(profile_role) = profile_meta
        .pointer("/metadata/role")
        .and_then(|v| v.as_str())
    {
        if let Some(job_role) = job_meta.pointer("/tags/role").and_then(|v| v.as_str()) {
            let similarity = jaro_winkler(profile_role, job_role);
            if similarity >= 0.8 {
                score *= 1.2;
                info!(
                    "🎯 Role roughly matches ('{}' ≈ '{}', similarity {:.2}) → boosted score to {:.4}",
                    profile_role, job_role, similarity, score
                );
            } else {
                score *= 0.85;
                mismatches += 1;
                info!(
                    "⚠️ Role mismatch ('{}' != '{}', similarity {:.2}) → penalized score to {:.4}",
                    profile_role, job_role, similarity, score
                );
            }
        } else {
            score *= 0.9;
            mismatches += 1;
            info!("⚠️ Missing job role → slight penalty to {:.4}", score);
        }
    }

    // --- INDUSTRY MATCH ---
    if let Some(profile_industry) = profile_meta
        .pointer("/metadata/industry")
        .and_then(|v| v.as_str())
    {
        if let Some(job_industry) = job_meta.pointer("/tags/industry").and_then(|v| v.as_str()) {
            let similarity = jaro_winkler(profile_industry, job_industry);
            if similarity >= 0.8 {
                score *= 1.1;
                info!(
                    "🏭 Industry roughly matches ('{}' ≈ '{}', similarity {:.2}) → boosted score to {:.4}",
                    profile_industry, job_industry, similarity, score
                );
            } else {
                score *= 0.9;
                mismatches += 1;
                info!(
                    "⚠️ Industry mismatch ('{}' != '{}', similarity {:.2}) → penalized score to {:.4}",
                    profile_industry, job_industry, similarity, score
                );
            }
        } else {
            score *= 0.95;
            mismatches += 1;
            info!("⚠️ Missing job industry → slight penalty to {:.4}", score);
        }
    }

    // --- LOCATION MATCH ---
    if let Some(profile_city) = profile_meta
        .pointer("/metadata/whoIAm/locationData/city")
        .and_then(|v| v.as_str())
    {
        if let Some(job_city) = job_meta
            .pointer("/tags/jobProviderLocation/city")
            .and_then(|v| v.as_str())
        {
            let similarity = jaro_winkler(profile_city, job_city);
            if similarity >= 0.8 {
                score *= 1.05;
                info!(
                    "📍 Location roughly matches ('{}' ≈ '{}', similarity {:.2}) → boosted score to {:.4}",
                    profile_city, job_city, similarity, score
                );
            } else {
                score *= 0.95;
                mismatches += 1;
                info!(
                    "⚠️ Location mismatch ('{}' != '{}', similarity {:.2}) → penalized score to {:.4}",
                    profile_city, job_city, similarity, score
                );
            }
        } else {
            score *= 0.95;
            mismatches += 1;
            info!("⚠️ Missing job location → slight penalty to {:.4}", score);
        }
    }

    // --- Additional penalties for multiple mismatches ---
    match mismatches {
        2 => {
            score *= 0.85;
            info!(
                "⚠️ 2 mismatches → additional moderate penalty applied, score {:.4}",
                score
            );
        }
        3..=usize::MAX => {
            score *= 0.7;
            info!(
                "🚨 3+ mismatches → additional heavy penalty applied, score {:.4}",
                score
            );
        }
        _ => {}
    }

    // --- Normalize ---
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
