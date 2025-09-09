use chrono::Utc;
use chrono_tz::Asia::Kolkata;
use std::fs;
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{
    fmt as tracing_fmt,
    fmt::{format::Writer, time::FormatTime},
    prelude::*,
    EnvFilter,
};

// -----------------------
// Custom India Time
// -----------------------
struct IndiaTime;

impl FormatTime for IndiaTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = Utc::now().with_timezone(&Kolkata);
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S"))
    }
}

// -----------------------
// Logging Setup
// -----------------------
pub fn setup_logging(log_dir: &str, svc: &str) -> (WorkerGuard, WorkerGuard, WorkerGuard) {
    // -----------------------
    // Normal Logs
    // -----------------------
    let normal_log_dir = format!("{}/{}", log_dir, svc);
    fs::create_dir_all(&normal_log_dir).expect("Failed to create normal log directory");
    let normal_file_name = format!("{}.log", svc);
    let (normal_writer, normal_guard) =
        tracing_appender::non_blocking(rolling::daily(normal_log_dir, normal_file_name));

    let normal_layer = tracing_fmt::layer()
        .with_writer(normal_writer)
        .json()
        .with_timer(IndiaTime)
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(EnvFilter::new("info"));

    // -----------------------
    // Performance Logs
    // -----------------------
    let perf_log_dir = format!("{}/perf", log_dir);
    fs::create_dir_all(&perf_log_dir).expect("Failed to create perf log directory");
    let perf_file_name = format!("{}_perf.log", svc);
    let (perf_writer, perf_guard) =
        tracing_appender::non_blocking(rolling::daily(perf_log_dir, perf_file_name));

    use tracing_subscriber::filter::Targets;
    let targets = Targets::new().with_target("perf", tracing::Level::INFO);

    let perf_layer = tracing_fmt::layer()
        .with_writer(perf_writer)
        .json()
        .with_timer(IndiaTime)
        .with_target(true)
        .with_filter(targets);

    // -----------------------
    // Cron Logs
    // -----------------------
    let cron_log_dir = format!("{}/cron", log_dir);
    fs::create_dir_all(&cron_log_dir).expect("Failed to create cron log directory");
    let cron_file_name = format!("{}_cron.log", svc);
    let (cron_writer, cron_guard) =
        tracing_appender::non_blocking(rolling::daily(cron_log_dir, cron_file_name));

    let cron_layer = tracing_fmt::layer()
        .with_writer(cron_writer)
        .json()
        .with_timer(IndiaTime)
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(EnvFilter::new("cron=info"));

    // -----------------------
    // Console Layer
    // -----------------------
    let console_layer = tracing_fmt::layer()
        .compact()
        .with_timer(IndiaTime)
        .with_target(true)
        .with_thread_ids(false)
        .with_filter(EnvFilter::new("info"));

    // -----------------------
    // Set Global Subscriber
    // -----------------------
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(normal_layer)
            .with(perf_layer)
            .with(cron_layer)
            .with(console_layer),
    )
    .expect("Failed to set global subscriber");

    (normal_guard, perf_guard, cron_guard)
}
