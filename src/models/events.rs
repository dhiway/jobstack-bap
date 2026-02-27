use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct EventRequest {
    pub event_type: EventType,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEvent {
    pub id: Uuid,
    pub event_type: EventType,
    pub payload: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct EventResponse {
    pub status: String,
    pub event_id: Uuid,
    pub stream: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "profile.updated")]
    ProfileUpdated,
    #[serde(rename = "profile.created")]
    ProfileCreated,
}
