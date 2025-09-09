use crate::config::AppConfig;
use tracing::info;

pub async fn run(app_config: AppConfig) {
    // log start
    info!(target: "cron", "🔄 Starting fetch jobs cron...");

    // // Example: call your service logic
    // if let Err(err) = search::fetch_and_store_jobs(app_state.clone()).await {
    //     tracing::error!("Fetch jobs failed: {:?}", err);
    // }

    info!(target: "cron", "✅ Fetch jobs cron finished.");
}
