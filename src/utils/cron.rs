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
