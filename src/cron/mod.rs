use crate::state::AppState;
use crate::utils::cron::build_cron_expr;
use tokio::time::{sleep, Duration};
use tokio_cron_scheduler::{Job, JobScheduler};

mod fetch_jobs;
mod fetch_profiles;

pub async fn start_cron_jobs(state: AppState) -> JobScheduler {
    let scheduler = JobScheduler::new().await.unwrap();

    /*
     * ------------------------------------------------------------
     * Initial delayed run after server restart
     * ------------------------------------------------------------
     */
    {
        let state = state.clone();
        tokio::spawn(async move {
            tracing::info!("🚀 Server restarted, waiting 5 seconds before first fetch_jobs...");
            sleep(Duration::from_secs(5)).await;

            tracing::info!("📦 Running initial fetch_jobs...");
            fetch_jobs::run(state).await;
        });
    }
    {
        let state = state.clone();
        tokio::spawn(async move {
            tracing::info!(
                "🚀 Server restarted, waiting 30 seconds before first fetch_profiles..."
            );
            sleep(Duration::from_secs(10)).await;

            tracing::info!("👤 Running initial fetch_profiles...");
            fetch_profiles::run(state).await;
        });
    }

    /*
     * ------------------------------------------------------------
     * fetch_jobs cron
     * ------------------------------------------------------------
     */

    let (jobs_desc, jobs_cron_expr) = build_cron_expr(state.config.cron.fetch_jobs.seconds);

    tracing::info!(
        "📅 Scheduling fetch_jobs cron: {} → {}",
        jobs_desc,
        jobs_cron_expr
    );

    scheduler
        .add(
            Job::new_async(&jobs_cron_expr, {
                let state = state.clone();
                move |_uuid, _l| {
                    let state = state.clone();
                    Box::pin(async move {
                        fetch_jobs::run(state).await;
                    })
                }
            })
            .unwrap(),
        )
        .await
        .unwrap();

    /*
     * ------------------------------------------------------------
     * fetch_profiles cron
     * ------------------------------------------------------------
     */

    let (profiles_desc, profiles_cron_expr) =
        build_cron_expr(state.config.cron.fetch_profiles.seconds);

    tracing::info!(
        "📅 Scheduling fetch_profiles cron: {} → {}",
        profiles_desc,
        profiles_cron_expr
    );

    scheduler
        .add(
            Job::new_async(&profiles_cron_expr, {
                let state = state.clone();
                move |_uuid, _l| {
                    let state = state.clone();
                    Box::pin(async move {
                        fetch_profiles::run(state).await;
                    })
                }
            })
            .unwrap(),
        )
        .await
        .unwrap();

    /*
     * ------------------------------------------------------------
     * Start scheduler
     * ------------------------------------------------------------
     */

    scheduler.start().await.unwrap();
    scheduler
}
