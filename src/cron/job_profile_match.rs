use crate::state::AppState;
use crate::utils::match_score::calculate_match_score;
use tracing::info;

pub async fn run(app_state: AppState) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 starting match score cron.             ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    calculate_match_score(&app_state).await;
}
