use crate::state::AppState;
use crate::utils::search::send_open_jobs_search;
use std::sync::Arc;
use tracing::info;

pub async fn run(app_state: Arc<AppState>) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 Starting fetch jobs cron.             ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    send_open_jobs_search(&app_state, 1, 30, "cron", None, None, None).await;
}
