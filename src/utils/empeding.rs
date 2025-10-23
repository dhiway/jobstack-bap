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
        if let crate::config::MatchMode::Embed = field.match_mode {
            if let Some(value) = profile.pointer(&field.profile_path) {
                let weight = field.weight.unwrap_or(1);
                if field.is_array {
                    if let Some(arr) = value.as_array() {
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                weighted_push(&mut parts, s, weight);
                            }
                        }
                    }
                } else if let Some(s) = value.as_str() {
                    weighted_push(&mut parts, s, weight);
                }
            }
        }
    }

    parts.join(" ")
}

pub fn job_text_for_embedding(job: &Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();

    for field in &config.metadata_match {
        if let crate::config::MatchMode::Embed = field.match_mode {
            if let Some(value) = job.pointer(&field.job_path) {
                let weight = field.weight.unwrap_or(1);
                if field.is_array {
                    if let Some(arr) = value.as_array() {
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                weighted_push(&mut parts, s, weight);
                            }
                        }
                    }
                } else if let Some(s) = value.as_str() {
                    weighted_push(&mut parts, s, weight);
                }
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

/// Compute final match score combining embedding cosine and manual numeric fields
pub fn compute_match_score(
    profile_emb: &[f32],
    job_emb: &[f32],
    profile_meta: &Value,
    job_meta: &Value,
    config: &AppConfig,
) -> f32 {
    info!("🔍 Computing match score...");

    // Base cosine similarity
    let mut score = cosine_similarity(profile_emb, job_emb);
    let base_score = score;
    info!("🧮 Base cosine similarity score: {:.4}", base_score);

    let mut mismatches = 0;

    for field in &config.metadata_match {
        // Common values
        let profile_val = profile_meta.pointer(&field.profile_path);
        let job_val = job_meta.pointer(&field.job_path);

        // 👇 Generic missing profile penalty
        if job_val.is_some() && (profile_val.is_none() || profile_val == Some(&Value::Null)) {
            score *= field.penalty;
            mismatches += 1;
            info!(
                "⚠️ {} present in job but missing in profile → applied penalty {:.2}, score now {:.4}",
                field.name, field.penalty, score
            );
        }

        match field.match_mode {
            crate::config::MatchMode::Embed => {
                // String similarity for role/industry
                if field.name == "role" || field.name == "industry" {
                    let profile_str = profile_val.and_then(|v| v.as_str()).unwrap_or_default();
                    let job_str = job_val.and_then(|v| v.as_str()).unwrap_or_default();

                    if !profile_str.is_empty() && !job_str.is_empty() {
                        let sim = jaro_winkler(profile_str, job_str);
                        if sim < 0.8 {
                            score *= field.penalty;
                            mismatches += 1;
                            info!(
                                "⚠️ {} similarity low ({:.2}) → applied penalty {:.2}, score now {:.4}",
                                field.name, sim, field.penalty, score
                            );
                        } else {
                            info!(
                                "✅ {} similarity good ({:.2}) → no penalty applied",
                                field.name, sim
                            );
                        }
                    }
                }
            }

            crate::config::MatchMode::Manual => {
                // Range-based manual fields (like salary, hours, etc.)
                let job_min = field
                    .job_path_min
                    .as_ref()
                    .and_then(|p| job_meta.pointer(p));
                let job_max = field
                    .job_path_max
                    .as_ref()
                    .and_then(|p| job_meta.pointer(p));

                if let (Some(profile_val), Some(job_min), Some(job_max)) =
                    (profile_val, job_min, job_max)
                {
                    if let (Some(p), Some(min), Some(max)) =
                        (profile_val.as_f64(), job_min.as_f64(), job_max.as_f64())
                    {
                        if p < min || p > max {
                            score *= field.penalty;
                            mismatches += 1;
                            info!(
                                "⚠️ {} out of range ({} not in [{}, {}]) → applied penalty {:.2}, score now {:.4}",
                                field.name, p, min, max, field.penalty, score
                            );
                        } else if let Some(bonus) = field.bonus {
                            score *= bonus;
                            info!(
                                "✅ {} in range ({} in [{}, {}]) → applied bonus {:.2}, score now {:.4}",
                                field.name, p, min, max, bonus, score
                            );
                        }
                    }
                }
            }
        }
    }

    // Additional global penalties
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
