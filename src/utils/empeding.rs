use tracing::info;

pub fn job_text_for_embedding(job: &serde_json::Value) -> String {
    info!("Generating text for job embedding: {:?}", job.get("id"));
    let mut parts = Vec::new();

    // Title from descriptor
    if let Some(title) = job.pointer("/descriptor/name").and_then(|v| v.as_str()) {
        parts.push(title);
    }

    // Company / job provider name
    if let Some(company) = job
        .pointer("/tags/basicInfo/jobProviderName")
        .and_then(|v| v.as_str())
    {
        parts.push(company);
    }

    // Industry
    if let Some(industry) = job.pointer("/tags/industry").and_then(|v| v.as_str()) {
        parts.push(industry);
    }

    // Role
    if let Some(role) = job.pointer("/tags/role").and_then(|v| v.as_str()) {
        parts.push(role);
    }

    // Location (city)
    if let Some(location) = job
        .pointer("/tags/jobProviderLocation/city")
        .and_then(|v| v.as_str())
    {
        parts.push(location);
    }

    parts.join(" ")
}

pub fn profile_text_for_embedding(profile: &serde_json::Value) -> String {
    let mut parts = Vec::new();

    // Basic info
    if let Some(name) = profile.pointer("/metadata/name").and_then(|v| v.as_str()) {
        parts.push(name.to_string());
    }
    if let Some(age) = profile.pointer("/metadata/age").and_then(|v| v.as_u64()) {
        let age_str = age.to_string();
        parts.push(age_str);
    }

    // Who I Am
    if let Some(who) = profile.pointer("/metadata/whoiam") {
        if let Some(location) = who.get("location").and_then(|v| v.as_str()) {
            parts.push(location.to_string());
        }
        if let Some(city) = who.pointer("/locationData/city").and_then(|v| v.as_str()) {
            parts.push(city.to_string());
        }
        if let Some(state) = who.pointer("/locationData/state").and_then(|v| v.as_str()) {
            parts.push(state.to_string());
        }
    }

    // Skills / ITI Specialization
    if let Some(specializations) = profile
        .pointer("/metadata/whatihave/itiSpecialization")
        .and_then(|v| v.as_array())
    {
        for s in specializations {
            if let Some(s) = s.as_str() {
                parts.push(s.to_string());
            }
        }
    }

    // Languages
    if let Some(languages) = profile
        .pointer("/metadata/whatihave/languageSpoken")
        .and_then(|v| v.as_array())
    {
        for l in languages {
            if let Some(l) = l.as_str() {
                parts.push(l.to_string());
            }
        }
    }

    // Previous company, institute
    if let Some(prev) = profile
        .pointer("/metadata/whatihave/previousCompany")
        .and_then(|v| v.as_str())
    {
        parts.push(prev.to_string());
    }
    if let Some(institute) = profile
        .pointer("/metadata/whatihave/itiInstitute")
        .and_then(|v| v.as_str())
    {
        parts.push(institute.to_string());
    }

    // Education, workExperience, certificates
    if let Some(education) = profile
        .pointer("/metadata/education")
        .and_then(|v| v.as_array())
    {
        for e in education {
            if let Some(s) = e.as_str() {
                parts.push(s.to_string());
            }
        }
    }

    if let Some(work_exp) = profile
        .pointer("/metadata/workExperience")
        .and_then(|v| v.as_array())
    {
        for w in work_exp {
            if let Some(s) = w.as_str() {
                parts.push(s.to_string());
            }
        }
    }

    if let Some(certificates) = profile
        .pointer("/metadata/certificates")
        .and_then(|v| v.as_array())
    {
        for c in certificates {
            if let Some(s) = c.as_str() {
                parts.push(s.to_string());
            }
        }
    }

    // What I Want
    if let Some(preferences) = profile
        .pointer("/metadata/whatiwant")
        .and_then(|v| v.as_object())
    {
        for (_k, v) in preferences {
            if v.is_string() {
                if let Some(s) = v.as_str() {
                    parts.push(s.to_string());
                }
            } else if v.is_number() {
                parts.push(v.to_string());
            } else if v.is_array() {
                for s in v.as_array().unwrap() {
                    if let Some(s) = s.as_str() {
                        parts.push(s.to_string());
                    }
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
