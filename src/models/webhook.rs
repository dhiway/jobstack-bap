use crate::models::core::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub timestamp: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookPayload {
    pub context: Context,
    pub message: Value,
}

#[derive(Debug, Serialize)]
pub struct AckResponse {
    pub message: AckStatus,
}

#[derive(Debug, Serialize)]
pub struct AckStatus {
    pub ack: Ack,
}

#[derive(Debug, Serialize)]
pub struct Ack {
    pub status: &'static str,
}
