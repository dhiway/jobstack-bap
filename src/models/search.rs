use serde::{Deserialize, Serialize};

use crate::models::core::{Location, Tag};

#[derive(Debug, Serialize, Deserialize)]
pub struct Skill {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<Skill>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<Person>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fulfillment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemDescriptor {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<ItemDescriptor>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Provider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<ProviderDescriptor>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Intent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<Fulfillment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchMessage {
    pub intent: Intent,
    pub pagination: Option<Pagination>,
    pub options: Option<Options>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub message: SearchMessage,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequestV2 {
    pub provider: Option<String>,
    pub role: Option<String>,
    pub query: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub primary_filters: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub exclude: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Options {
    pub breif: Option<bool>,
}
