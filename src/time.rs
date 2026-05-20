use chrono::{Days, Local, NaiveDate};

pub fn todays_date() -> NaiveDate {
    Local::now().date_naive()
}

pub fn yesterdays_date() -> NaiveDate {
    let today = Local::now().date_naive();
    today.checked_sub_days(Days::new(1)).unwrap()
}
