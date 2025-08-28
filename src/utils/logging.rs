use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, fmt::time::UtcTime, prelude::*, EnvFilter};

pub fn setup_logging(log_dir: &str, svc: &str) -> WorkerGuard {
    let log_file_name = format!("{}.log", svc);

    let (file_writer, file_guard) =
        tracing_appender::non_blocking(rolling::daily(log_dir, log_file_name));

    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .json()
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(EnvFilter::new("info"));

    let console_layer = fmt::layer()
        .compact()
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(EnvFilter::new("info"));

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer),
    )
    .expect("Failed to set global subscriber");

    file_guard
}
