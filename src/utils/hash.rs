use crate::models::search::SearchMessage;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn generate_query_hash(intent: &SearchMessage) -> String {
    let input = serde_json::to_string(intent).expect("intent is always serializable");
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn hash_json(value: &Value) -> String {
    let canonical = serde_json::to_vec(value).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    format!("{:x}", hasher.finalize())
}
