use serde_json::Value as JsonValue;

pub fn matches_query_dynamic(provider_name: &str, item: &JsonValue, qf: &str) -> bool {
    // Split by comma and normalize
    let queries: Vec<String> = qf
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    for q in queries {
        if provider_name.to_lowercase().contains(&q) {
            return true;
        }

        // role / descriptor
        if item
            .pointer("/descriptor/name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // locations
        if let Some(location) = item.get("locations") {
            if location.to_string().to_lowercase().contains(&q) {
                return true;
            }
        }

        // tags.industry
        if item
            .pointer("/tags/industry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.role
        if item
            .pointer("/tags/role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.jobDetails.title
        if item
            .pointer("/tags/jobDetails/title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains(&q))
            .unwrap_or(false)
        {
            return true;
        }

        // tags.jobProviderLocation
        if let Some(loc) = item.pointer("/tags/jobProviderLocation") {
            if loc.to_string().to_lowercase().contains(&q) {
                return true;
            }
        }
    }

    false
}
