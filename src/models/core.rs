use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct Context {
    pub domain: String,
    pub action: String,
    pub version: String,
    pub bap_id: String,
    pub bap_uri: String,
    pub transaction_id: String,
    pub message_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpp_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpp_uri: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MinimalContext {
    pub transaction_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Message {
    Intent { intent: Value },
    Order { order: Value },
}

// --- Location structs
#[derive(Debug, Serialize, Deserialize)]
pub struct Gps {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct City {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Country {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    pub gps: Gps,
    pub address: String,
    pub city: City,
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<Country>,
}

// --- Tag structs
#[derive(Debug, Serialize, Deserialize)]
pub struct TagItem {
    pub descriptor: Descriptor,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Descriptor {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub descriptor: Descriptor,
    pub list: Vec<TagItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fulfillment {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<FulfillmentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FulfillmentState {
    pub descriptor: Descriptor,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    pub person: Person,
    pub contact: Contact,
    pub location: Location,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<Skill>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<Language>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Skill {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Language {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
