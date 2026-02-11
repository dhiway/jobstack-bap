use crate::config::NotificationSchedule;
use crate::utils::notification::{build_notification_cron_type, NotificationCronType};
pub fn build_cron_expr(seconds: u64) -> (String, String) {
    let desc = if seconds < 60 {
        format!("every {} seconds", seconds)
    } else if seconds % 60 == 0 {
        format!("every {} minutes", seconds / 60)
    } else {
        format!("every {} minutes {} seconds", seconds / 60, seconds % 60)
    };

    let expr = if seconds < 60 {
        format!("*/{} * * * * *", seconds)
    } else {
        format!("0 */{} * * * *", seconds / 60)
    };

    (desc, expr)
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
