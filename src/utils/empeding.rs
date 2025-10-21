use crate::config::AppConfig;
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
