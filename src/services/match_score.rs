use crate::{
    db::match_score::{JobRow, ProfileRow},
    state::AppState,
};
use serde_json::Value;

pub fn compute_match_score(
    app_state: &AppState,
    job: &JobRow,
    profile: &ProfileRow,
) -> (i16, Option<Value>) {
    let source = &app_state.config.cron.compute_match_scores.source;

    match source.as_str() {
        "empeding" => compute_match_score_empeding(job, profile),
        _ => compute_match_score_empeding(job, profile),
    }
}

pub fn compute_match_score_empeding(_job: &JobRow, _profile: &ProfileRow) -> (i16, Option<Value>) {
    (0, None)
}
