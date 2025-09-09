use crate::state::AppState;
use tokio::time::{sleep, Duration};
use tokio_cron_scheduler::{Job, JobScheduler};

mod fetch_jobs;

pub async fn start_cron_jobs(state: AppState) -> JobScheduler {
    let scheduler = JobScheduler::new().await.unwrap();

    // 👉 Wait 5 seconds and run first fetch after server start
    {
        let state = state.clone();
        tokio::spawn(async move {
            tracing::info!("🚀 Server restarted, waiting 10 seconds before first fetch...");
            sleep(Duration::from_secs(5)).await;

            tracing::info!("📦 Running initial fetch_jobs cron...");
            fetch_jobs::run(state).await;
        });
    }

    let fetch_jobs_seconds = state.config.cron.fetch_jobs.seconds;

    let schedule_desc = if fetch_jobs_seconds < 60 {
        format!(
            "every {} second{}",
            fetch_jobs_seconds,
            if fetch_jobs_seconds > 1 { "s" } else { "" }
        )
    } else if fetch_jobs_seconds % 60 == 0 {
        let minutes = fetch_jobs_seconds / 60;
        format!(
            "every {} minute{}",
            minutes,
            if minutes > 1 { "s" } else { "" }
        )
    } else {
        let minutes = fetch_jobs_seconds / 60;
        let seconds = fetch_jobs_seconds % 60;
        format!(
            "every {} minute{} {} second{}",
            minutes,
            if minutes > 1 { "s" } else { "" },
            seconds,
            if seconds > 1 { "s" } else { "" }
        )
    };

    let fetch_jobs_cron_expr = if fetch_jobs_seconds < 60 {
        format!("*/{} * * * * *", fetch_jobs_seconds)
    } else {
        let minutes = fetch_jobs_seconds / 60;
        format!("0 */{} * * * *", minutes)
    };

    tracing::info!(
        "📅 Scheduling fetch_jobs cron: {} → Cron expression: {}",
        schedule_desc,
        fetch_jobs_cron_expr
    );

    scheduler
        .add(
            Job::new_async(&fetch_jobs_cron_expr, {
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

    scheduler.start().await.unwrap();
    scheduler
}
