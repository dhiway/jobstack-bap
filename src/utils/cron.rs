use crate::config::NotificationSchedule;
use crate::utils::notification::{build_notification_cron_type, NotificationCronType};
pub fn build_cron_expr(seconds: u64) -> (String, String) {
    if seconds < 60 {
        (
            format!("every {} seconds", seconds),
            format!("*/{} * * * * *", seconds),
        )
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        (
            format!("every {} minutes", minutes),
            format!("0 */{} * * * *", minutes),
        )
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        (
            format!("every {} hours", hours),
            format!("0 0 */{} * * *", hours),
        )
    } else {
        ("once per day".to_string(), "0 0 0 * * *".to_string())
    }
}

pub fn build_notification_cron_expr(schedule: &NotificationSchedule) -> (String, String) {
    let cron_type = build_notification_cron_type(schedule);
    match cron_type {
        NotificationCronType::Weekly {
            weekday,
            hour,
            minute,
            seconds,
        } => {
            let desc = format!("Weekly on weekday {} at {:02}:{:02}", weekday, hour, minute);

            let expr = format!("{} {} {} * * {}", seconds, minute, hour, weekday);

            (desc, expr)
        }

        NotificationCronType::Monthly {
            day,
            hour,
            minute,
            seconds,
        } => {
            let desc = format!("Monthly on day {} at {:02}:{:02}", day, hour, minute);

            let expr = format!("{} {} {} {} * *", seconds, minute, hour, day);

            (desc, expr)
        }
    }
}
