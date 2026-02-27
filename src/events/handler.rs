use crate::models::events::{AppEvent, EventType};
use crate::services::profiles::sync_profile_by_id;
use crate::state::AppState;

use crate::events::utils::extract_profile_id;
use std::sync::Arc;
use tracing::info;
pub async fn handle_event(state: &Arc<AppState>, event: AppEvent) -> anyhow::Result<()> {
    info!(" Events {:?}", event);

    match event.event_type {
        EventType::ProfileCreated | EventType::ProfileUpdated => {
            let profile_id = extract_profile_id(&event)?;
            sync_profile_by_id(state, &profile_id).await?;
        }
    }

    Ok(())
}
