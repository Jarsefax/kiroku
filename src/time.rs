use chrono::Local;

pub fn todays_date() -> String {
    let today = Local::now().date_naive();
    today.format("%Y-%m-%d").to_string()
}
