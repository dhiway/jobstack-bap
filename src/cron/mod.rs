use crate::config::AppConfig;
use tokio_cron_scheduler::{Job, JobScheduler};

mod fetch_jobs;

pub async fn start_cron_jobs(config: AppConfig) -> JobScheduler {
    let scheduler = JobScheduler::new().await.unwrap();

    let fetch_jobs_seconds = config.cron.fetch_jobs.seconds;
    let fetch_jobs_cron_expr = format!("*/{} * * * * *", fetch_jobs_seconds);

    tracing::info!(
        "📅 Scheduling fetch_jobs cron with expression: {}",
        fetch_jobs_cron_expr
    );

    scheduler
        .add(
            Job::new_async(&fetch_jobs_cron_expr, {
                let config = config.clone();
                move |_uuid, _l| {
                    let config = config.clone();
                    Box::pin(async move {
                        fetch_jobs::run(config).await;
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
