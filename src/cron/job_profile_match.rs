use crate::state::AppState;
use crate::utils::match_score::calculate_match_score;
use std::sync::Arc;
use tracing::info;

pub async fn run(app_state: Arc<AppState>) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 starting match score cron.             ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    calculate_match_score(&app_state).await;
}
