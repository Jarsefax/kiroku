use crate::storage::{Day, Status, Store, parse_date};
use anyhow::{Result, bail, ensure};
use chrono::{Datelike, Duration, NaiveDate};
use std::fmt::Write;

pub struct View {
    pub days: Vec<Day>,
    pub active: Option<NaiveDate>,
    pub notice: String,
}

pub fn range(selector: &str, today: NaiveDate) -> Result<(NaiveDate, NaiveDate)> {
    let (from, to) = match selector {
        "today" => (today, today),
        "yesterday" => {
            let yesterday = today
                .checked_sub_signed(Duration::days(1))
                .ok_or_else(|| anyhow::anyhow!("Date out of range"))?;
            (yesterday, yesterday)
        }
        "week" => {
            let start = today - Duration::days(today.weekday().num_days_from_monday().into());
            (start, start + Duration::days(6))
        }
        "month" => {
            let start = today.with_day(1).unwrap();
            let next = if today.month() == 12 {
                NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
            };
            (
                start,
                next.ok_or_else(|| anyhow::anyhow!("Date out of range"))? - Duration::days(1),
            )
        }
        text if text.contains("..") => {
            let (from, to) = text.split_once("..").unwrap();
            (parse_date(from)?, parse_date(to)?)
        }
        text => {
            if text.is_empty() {
                bail!("Specify today, yesterday, week, month, YYYY-MM-DD, or FROM..TO");
            }
            let date = parse_date(text)?;
            (date, date)
        }
    };
    ensure!(from <= to, "The range start must not be later than its end");
    Ok((from, to))
}

pub fn select(store: &Store, selector: Option<&str>, today: NaiveDate) -> Result<View> {
    let active_day = store.active()?;
    let active = active_day.as_ref().map(|d| d.date);
    if let Some(selector) = selector {
        let days = store.days(Some(range(selector, today)?))?;
        let notice = if days.is_empty() {
            format!("No recorded days for {selector}.")
        } else {
            String::new()
        };
        return Ok(View {
            days,
            active,
            notice,
        });
    }
    let mut days = store.days(Some((today, today)))?;
    let notice;
    if let Some(day) = days.first() {
        if day.items.is_empty() {
            if let Some(previous) = store.previous(today)? {
                notice = "Today is empty; also showing the previous recorded day.".to_owned();
                days.push(previous);
            } else {
                notice = "Today is empty. No previous day has been recorded.".to_owned();
            }
        } else {
            notice = String::new();
        }
    } else if let Some(day) = active_day {
        notice = format!("Today ({today}) has not been started; showing the last recorded day.");
        days.push(day);
    } else {
        notice = format!(
            "No recorded days. Today ({today}) has not been started. Run `krk new-day` to start."
        );
    }
    Ok(View {
        days,
        active,
        notice,
    })
}

pub fn letters(mut index: usize) -> String {
    let mut result = vec![];
    loop {
        result.push((b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    result.into_iter().rev().collect()
}

pub fn render(view: &View, today: NaiveDate) -> String {
    let mut output = String::new();
    if !view.notice.is_empty() {
        let _ = writeln!(output, "{}\n", view.notice);
    }
    if view.days.is_empty() {
        if view.notice.is_empty() {
            output.push_str("No recorded days. Run `krk new-day` to start.\n");
        }
        return output;
    }
    let mut historical_index = 0;
    for day in &view.days {
        let is_active = Some(day.date) == view.active;
        let mut labels = vec![];
        if day.date == today {
            labels.push("today");
        }
        if is_active {
            labels.push("active — numbers are editable");
        } else {
            labels.push("history — letters are browsing labels");
        }
        let _ = writeln!(output, "{} ({})", day.date, labels.join("; "));
        let mut number = 1;
        for status in Status::ALL {
            let _ = writeln!(output, "  {}", status.label());
            let items: Vec<_> = day
                .numbered()
                .into_iter()
                .filter(|i| i.status == status)
                .collect();
            if items.is_empty() {
                output.push_str("    (empty)\n");
            }
            for item in items {
                let label = if is_active {
                    let label = number.to_string();
                    number += 1;
                    label
                } else {
                    let label = letters(historical_index);
                    historical_index += 1;
                    label
                };
                let _ = writeln!(output, "    {label}. {}", item.title);
            }
        }
        output.push('\n');
    }
    output
}
