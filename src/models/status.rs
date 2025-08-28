use crate::models::core::MinimalContext;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusRequest {
    pub context: MinimalContext,
    pub message: StatusMessage,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusMessage {
    pub order: StatusOrder,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatusOrder {
    pub id: String,
}
