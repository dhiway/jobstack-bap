use crate::models::core::MinimalContext;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize)]
pub struct SelectRequest {
    pub context: MinimalContext,
    pub message: SelectMessage,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SelectMessage {
    pub order: SelectOrder,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SelectOrder {
    pub provider: OrderProvider,
    pub items: Vec<OrderItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderProvider {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderItem {
    pub id: String,
}
