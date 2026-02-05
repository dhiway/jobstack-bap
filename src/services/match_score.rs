use crate::db::match_score::{JobRow, ProfileRow};
use serde_json::Value;

pub fn compute_match_score(_job: &JobRow, _profile: &ProfileRow) -> (i16, Option<Value>) {
    // placeholder logic
    (0, None)
}
