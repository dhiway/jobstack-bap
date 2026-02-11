use crate::db::match_score::fetch_high_match_scores;
use crate::state::AppState;
use crate::utils::batching::chunk_vec;
use crate::utils::notification::send_whatsapp_notification;

use tokio::time::{sleep, Duration};
use tracing::{error, info};

pub async fn run(app_state: AppState) {
    info!(target: "cron", "╔════════════════════════════════════════════╗");
    info!(target: "cron", "║   🔄 Starting Notification cron.           ║");
    info!(target: "cron", "╚════════════════════════════════════════════╝");

    let result = fetch_high_match_scores(
        &app_state.db_pool,
        app_state.config.cron.notification.min_score,
    )
    .await;

    let high_matches = match result {
        Ok(rows) => rows,
        Err(e) => {
            error!(target: "cron", "Failed to fetch high matches: {:?}", e);
            return;
        }
    };

    if high_matches.is_empty() {
        info!(target: "cron", "No high matches found.");
        return;
    }

    let batch_size = app_state.config.cron.notification.batch.max(1);

    let batches = chunk_vec(high_matches, batch_size);
    let total_batches = batches.len();

    info!(
        target: "cron",
        "📦 Processing {} batches (batch_size={})",
        total_batches,
        batch_size
    );

    for (index, batch) in batches.into_iter().enumerate() {
        info!(
            target: "cron",
            "🚀 Processing batch {}/{}",
            index + 1,
            total_batches
        );

        for row in batch {
            let phone = match &row.phone {
                Some(p) => p,
                None => continue,
            };

            if let Err(e) = send_whatsapp_notification(
                &app_state,
                phone,
                row.name.as_deref().unwrap_or("User"),
                row.role.as_deref().unwrap_or("Job"),
                row.job_provider_name.as_deref().unwrap_or("Company"),
            )
            .await
            {
                error!(
                    target: "cron",
                    "Failed to send notification to {}: {:?}",
                    phone,
                    e
                );
            }
        }
        if index + 1 < total_batches {
            info!(
                target: "cron",
                "⏳ Sleeping {} s before next batch...",
                5
            );
            sleep(Duration::from_secs(5)).await;
        }
    }

    info!(target: "cron", "✅ Notification cron completed.");
}
