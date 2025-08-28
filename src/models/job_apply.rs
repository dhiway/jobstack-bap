use crate::models::core::{Fulfillment, MinimalContext};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct JobRequest {
    pub context: MinimalContext,
    pub message: InitMessage,
}

pub type DraftRequest = JobRequest;
pub type JobApplyRequest = JobRequest;

#[derive(Debug, Serialize, Deserialize)]
pub struct InitMessage {
    pub order: InitOrder,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitOrder {
    pub provider: Provider,
    pub items: Vec<InitOrderItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillments: Option<Vec<Fulfillment>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitOrderItem {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct JobApplications {
    pub user_id: String,
}

pub type JobApplicationsQuery = JobApplications;
pub type DraftApplicationsQuery = JobApplications;
