use reqwest::Client;
use serde_json::Value;
use tracing::{error, info};
pub async fn post_json(url: &str, payload: Value) -> anyhow::Result<()> {
    let client = Client::new();
    let res = client.post(url).json(&payload).send().await?;
    info!(
        "Sending POST request to {} with payload: {:?}",
        url, payload
    );

    if res.status().is_success() {
        println!("ok response");
        Ok(())
    } else {
        let status = res.status();
        let body = res.text().await?;
        error!("❌ Error response: status={}, body={}", status, body);
        Err(anyhow::anyhow!("Failed with status {}: {}", status, body))
    }
}
