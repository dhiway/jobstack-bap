use tokio::{signal, sync::watch};
use tracing::info;

use bap_onest_lite::{
    config::AppConfig, http::http_server::start_http_server, utils::logging::setup_logging,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _guard = setup_logging("app/logs", "bap-webhook");
    let config = AppConfig::new()?;

    let (shutdown_tx, shutdown_rx) = watch::channel(());

    // 👇 Spawn Ctrl+C listener
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            if signal::ctrl_c().await.is_ok() {
                info!("🛑 Received Ctrl+C. Triggering shutdown...");
                let _ = shutdown_tx.send(());
            }
        }
    });

    let server = start_http_server(config, shutdown_rx).await?;

    let result = tokio::select! {
        res = server => res,
    };

    // 👇 Catch any crash or panic
    if let Err(e) = result {
        tracing::error!("💥 Server crashed: {:?}", e);
        let _ = shutdown_tx.send(());
    }

    Ok(())
}
