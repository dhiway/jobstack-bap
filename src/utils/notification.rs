use crate::config::NotificationSchedule;
use crate::config::ScheduleType;
use crate::state::AppState;
use crate::utils::http_client::post_json;
use anyhow::Result;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;
use sha2::Sha256;
use tracing::info;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub enum NotificationCronType {
    Weekly {
        weekday: u8,
        hour: u8,
        minute: u8,
        seconds: u8,
    },
    Monthly {
        day: u8,
        hour: u8,
        minute: u8,
        seconds: u8,
    },
}

pub async fn send_whatsapp_notification(
    app_state: &AppState,
    phone: &str,
    name: &str,
    role: &str,
    provider_name: &str,
) -> Result<()> {
    let base_url = &app_state.config.services.notification.base_url;
    let url = format!("{}/notify", base_url.trim_end_matches('/'));

    let content_sid = &app_state.config.services.notification.content_sid;

    let payload = json!({
        "channel": "whatsapp",
        "template_id": "other",
        "to": phone,
        "priority": "realtime",
        "variables": {
            "contentSid": content_sid,
            "contentVariables": {
                "1": name,
                "2": "https://getjob.onest.network/0/seeker?tab=discover",
                "3": role,
                "4": provider_name
            }
        }
    });

    let method = "POST";
    let path = "/notify";
    let timestamp = Utc::now().timestamp().to_string();

    let nonce_bytes: [u8; 16] = rand::random();
    let nonce = hex::encode(nonce_bytes);

    let base_string = format!("{}\n{}\n{}\n{}", method, path, timestamp, nonce);

    let secret = &app_state.config.services.notification.ns_secret;
    let key_id = &app_state.config.services.notification.ns_key_id;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(base_string.as_bytes());
    let signature = format!("v1={}", hex::encode(mac.finalize().into_bytes()));

    let mut headers = HeaderMap::new();
    headers.insert("X-NS-Key", HeaderValue::from_str(key_id)?);
    headers.insert("X-NS-Timestamp", HeaderValue::from_str(&timestamp)?);
    headers.insert("X-NS-Nonce", HeaderValue::from_str(&nonce)?);
    headers.insert("X-NS-Signature", HeaderValue::from_str(&signature)?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    info!("📤 Sending WhatsApp notification to {}", phone);

    post_json(&url, payload, Some(headers)).await?;

    Ok(())
}

pub fn build_notification_cron_type(schedule: &NotificationSchedule) -> NotificationCronType {
    match schedule.schedule_type {
        ScheduleType::Weekly => NotificationCronType::Weekly {
            weekday: schedule.weekday.unwrap_or(1),
            hour: schedule.hour,
            minute: schedule.minute,
            seconds: schedule.seconds,
        },

        ScheduleType::Monthly => NotificationCronType::Monthly {
            day: schedule.day.unwrap_or(1),
            hour: schedule.hour,
            minute: schedule.minute,
            seconds: schedule.seconds,
        },
    }
}
