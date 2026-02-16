use anyhow::{anyhow, Result};
use tracing::error;

use crate::models::events::AppEvent;

pub fn extract_profile_id(event: &AppEvent) -> Result<String> {
    match event.payload.get("profileId").and_then(|v| v.as_str()) {
        Some(profile_id) if !profile_id.is_empty() => Ok(profile_id.to_string()),
        _ => {
            error!(
                event_id = %event.id,
                "❌ Missing or invalid profileId in event payload"
            );
            Err(anyhow!("Missing or invalid profileId in event payload"))
        }
    }
}
