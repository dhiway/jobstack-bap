use crate::config::AppConfig;
use crate::config::MetaDataMatch;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use strsim::jaro_winkler;
use tracing::info;

pub fn cached_jaro(
    profile_str: &str,
    job_str: &str,
    cache: &mut HashMap<(String, String), f32>,
) -> f32 {
    let key = (profile_str.to_string(), job_str.to_string());
    if let Some(&sim) = cache.get(&key) {
        return sim;
    }
    let sim = jaro_winkler(profile_str, job_str) as f32;
    cache.insert(key, sim);
    sim
}

fn weighted_push(parts: &mut Vec<String>, text: &str, weight: usize) {
    for _ in 0..weight {
        parts.push(text.to_string());
    }
}

fn load_match_score_config(path: &str) -> Vec<MetaDataMatch> {
    let data = fs::read_to_string(path).expect("Failed to read match_score.json");

    #[derive(serde::Deserialize)]
    struct Wrapper {
        match_score: Vec<MetaDataMatch>,
    }

    let wrapper: Wrapper = serde_json::from_str(&data).expect("Failed to parse match_score.json");
    wrapper.match_score
}

pub fn profile_text_for_embedding(profile: &Value, config: &AppConfig) -> String {
    let mut parts = Vec::new();
    let match_score = load_match_score_config(config.match_score_path.as_str());

    for field in &match_score {
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
    let match_score = load_match_score_config(config.match_score_path.as_str());

    let mut parts = Vec::new();

    for field in &match_score {
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

pub fn cosine_similarity_with_norm(vec_a: &[f32], vec_b: &[f32], norm_a: f32, norm_b: f32) -> f32 {
    if vec_a.len() != vec_b.len() || vec_a.is_empty() || norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    let dot_product: f32 = vec_a.iter().zip(vec_b.iter()).map(|(a, b)| a * b).sum();
    dot_product / (norm_a * norm_b)
}

/// Compute final match score combining embedding cosine and manual numeric fields
pub fn compute_empeding_match_score(
    profile_emb: &[f32],
    profile_norm: f32,
    job_emb: &[f32],
    job_norm: f32,
    profile_meta: &Value,
    job_meta: &Value,
    config: &AppConfig,
    string_sim_cache: &mut HashMap<(String, String), f32>,
) -> f32 {
    info!("🔍 Computing match score...");
    let match_score = load_match_score_config(config.match_score_path.as_str());

    // Base cosine similarity using precomputed norms
    let mut score = cosine_similarity_with_norm(profile_emb, job_emb, profile_norm, job_norm);
    let base_score = score;
    info!("🧮 Base cosine similarity score: {:.4}", base_score);

    let mut mismatches = 0;

    for field in &match_score {
        let profile_val = profile_meta.pointer(&field.profile_path);
        let job_val = job_meta.pointer(&field.job_path);

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
                if field.name == "role" || field.name == "industry" {
                    let profile_str = profile_val.and_then(|v| v.as_str()).unwrap_or_default();
                    let job_str = job_val.and_then(|v| v.as_str()).unwrap_or_default();

                    if !profile_str.is_empty() && !job_str.is_empty() {
                        let sim = cached_jaro(profile_str, job_str, string_sim_cache);
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

    match mismatches {
        2 => score *= 0.85,
        3..=usize::MAX => score *= 0.7,
        _ => {}
    }

    if score.is_nan() {
        score = 0.0;
        info!("🚫 NaN detected — setting score to 0.0");
    }

    score.clamp(0.0, 1.0)
}
