use crate::config::AppConfig;
use serde_json::Value;
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

    // info!(
    //     "📄 Profile metadata: {}",
    //     serde_json::to_string_pretty(profile_meta).unwrap_or_default()
    // );
    // info!(
    //     "💼 Job metadata: {}",
    //     serde_json::to_string_pretty(job_meta).unwrap_or_default()
    // );

    let mut score = cosine_similarity(profile_emb, job_emb);
    let base_score = score;
    info!("🧮 Base cosine similarity score: {:.4}", base_score);

    // --- ROLE BOOST / PENALTY ---
    if let Some(profile_role) = profile_meta
        .pointer("/metadata/role")
        .and_then(|v| v.as_str())
    {
        if let Some(job_role) = job_meta.pointer("/tags/role").and_then(|v| v.as_str()) {
            if profile_role.eq_ignore_ascii_case(job_role) {
                score *= 1.2;
                info!(
                    "🎯 Role match! '{}' == '{}' → boosted score to {:.4}",
                    profile_role, job_role, score
                );
            } else {
                score *= 0.85;
                info!(
                    "⚠️ Role mismatch: '{}' != '{}' → penalized score to {:.4}",
                    profile_role, job_role, score
                );
            }
        } else {
            score *= 0.9;
            info!("⚠️ Missing job role → slight penalty to {:.4}", score);
        }
    } else {
        info!("⚠️ No role in profile metadata — skipping role boost");
    }

    // --- INDUSTRY BOOST / PENALTY ---
    if let Some(profile_industry) = profile_meta
        .pointer("/metadata/industry")
        .and_then(|v| v.as_str())
    {
        if let Some(job_industry) = job_meta.pointer("/tags/industry").and_then(|v| v.as_str()) {
            if profile_industry.eq_ignore_ascii_case(job_industry) {
                score *= 1.1;
                info!(
                    "🏭 Industry match! '{}' == '{}' → boosted score to {:.4}",
                    profile_industry, job_industry, score
                );
            } else {
                score *= 0.9;
                info!(
                    "⚠️ Industry mismatch: '{}' != '{}' → penalized score to {:.4}",
                    profile_industry, job_industry, score
                );
            }
        } else {
            score *= 0.95;
            info!("⚠️ Missing job industry → slight penalty to {:.4}", score);
        }
    } else {
        info!("⚠️ No industry in profile metadata — skipping industry boost");
    }

    // --- LOCATION BOOST / PENALTY ---
    if let Some(profile_city) = profile_meta
        .pointer("/metadata/whoIAm/locationData/city")
        .and_then(|v| v.as_str())
    {
        if let Some(job_city) = job_meta
            .pointer("/tags/jobProviderLocation/city")
            .and_then(|v| v.as_str())
        {
            if profile_city.eq_ignore_ascii_case(job_city) {
                score *= 1.05;
                info!(
                    "📍 Location match! '{}' == '{}' → boosted score to {:.4}",
                    profile_city, job_city, score
                );
            } else {
                score *= 0.95;
                info!(
                    "⚠️ Location mismatch: '{}' != '{}' → penalized score to {:.4}",
                    profile_city, job_city, score
                );
            }
        } else {
            score *= 0.95;
            info!("⚠️ Missing job location → slight penalty to {:.4}", score);
        }
    } else {
        info!("⚠️ No city in profile metadata — skipping location boost");
    }

    // --- NORMALIZE ---
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
