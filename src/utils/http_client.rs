use reqwest::{header, Client};
use serde_json::Value;
use tracing::{error, info};

pub async fn post_json(
    url: &str,
    payload: Value,
    headers: Option<header::HeaderMap>,
) -> anyhow::Result<()> {
    let client = Client::new();

    info!("Sending POST request to {}", url);

    let mut request = client.post(url).json(&payload);

    if let Some(h) = headers {
        request = request.headers(h);
    }

    let res = request.send().await?;

    if res.status().is_success() {
        info!("✅ OK response");
        Ok(())
    } else {
        let status = res.status();
        let body = res.text().await?;
        error!("❌ Error response: status={}, body={}", status, body);
        Err(anyhow::anyhow!("Failed with status {}: {}", status, body))
    }
}

pub async fn get_json(url: &str, headers: header::HeaderMap) -> anyhow::Result<Value> {
    let client = Client::new();

    info!("Sending GET request to {}", url);

    let res = client.get(url).headers(headers).send().await?;

    if res.status().is_success() {
        Ok(res.json::<Value>().await?)
    } else {
        let status = res.status();
        let body = res.text().await?;
        error!("❌ Error response: status={}, body={}", status, body);
        Err(anyhow::anyhow!("Failed with status {}: {}", status, body))
    }
}
