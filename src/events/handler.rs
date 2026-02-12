use crate::models::events::{AppEvent, EventType};
use tracing::info;
pub async fn handle_event(event: AppEvent) -> anyhow::Result<()> {
    info!(" Events {:?}", event);

    match event.event_type {
        EventType::ProfileUpdated => {
            info!("Handle profile updated");
        }
        EventType::ProfileCreated => {
            info!("Handle profile created");
        }
    }

    Ok(())
}
