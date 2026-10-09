use crate::storage::{Day, Item, Status, Store, parse_date};
use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, Duration, NaiveDate};
use std::fmt::Write;

pub struct View {
    pub days: Vec<Day>,
    pub active: Option<NaiveDate>,
    pub notice: String,
}

impl View {
    /// Select the same one-based result number displayed by a CLI listing.
    pub fn result(&self, number: usize, filter: Option<&Filter>) -> Result<(&Day, usize)> {
        ensure!(number > 0, "Result numbers start at 1");
        self.days
            .iter()
            .flat_map(|day| {
                day.numbered()
                    .into_iter()
                    .enumerate()
                    .filter(move |(_, item)| filter.is_none_or(|filter| filter.matches(item)))
                    .map(move |(index, _)| (day, index + 1))
            })
            .nth(number - 1)
            .with_context(|| {
                format!("No result {number}; rerun the search or listing to see current results")
            })
    }
}

#[derive(Clone, Default)]
pub struct Filter {
    pub query: String,
    pub status: Option<Status>,
}

impl Filter {
    pub fn new(query: &str, status: Option<Status>) -> Result<Self> {
        ensure!(
            !query.chars().any(char::is_control),
            "Search text must be a single line without control characters"
        );
        Ok(Self {
            query: query.trim().to_lowercase(),
            status,
        })
    }

    pub fn matches(&self, item: &Item) -> bool {
        self.status.is_none_or(|status| status == item.status)
            && item.title.to_lowercase().contains(&self.query)
    }
}

/// Keep complete source days so filtered results retain their original labels.
pub fn select_filtered(
    store: &Store,
    selector: Option<&str>,
    today: NaiveDate,
    filter: &Filter,
) -> Result<View> {
    let active = store.active()?.map(|day| day.date);
    let mut days = store.days(selector.map(|text| range(text, today)).transpose()?)?;
    days.retain(|day| day.items.iter().any(|item| filter.matches(item)));
    let count = days
        .iter()
        .flat_map(|day| &day.items)
        .filter(|item| filter.matches(item))
        .count();
    let mut notice = if count == 0 {
        "No matching items.".to_owned()
    } else {
        let noun = if count == 1 { "item" } else { "items" };
        format!("{count} matching {noun}. Historical letters are scoped to each date.")
    };
    if let Some(active) = active {
        let _ = write!(notice, "\nCopies go to active day {active}.");
        if active != today {
            notice.push_str(" Run `krk new-day` to make today active.");
        }
    }
    Ok(View {
        days,
        active,
        notice,
    })
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

/// Convert a date-scoped historical label (A, B, ..., AA) to a one-based number.
pub fn letter_number(label: &str) -> Result<usize> {
    ensure!(
        !label.is_empty() && label.bytes().all(|b| b.is_ascii_uppercase()),
        "Historical items use uppercase letters from `krk list YYYY-MM-DD`"
    );
    label.bytes().try_fold(0usize, |number, byte| {
        number
            .checked_mul(26)
            .and_then(|n| n.checked_add(usize::from(byte - b'A') + 1))
            .context("Historical item label is too large")
    })
}

pub fn render(view: &View, today: NaiveDate) -> String {
    render_view(view, today, None, false)
}

pub fn render_list(view: &View, today: NaiveDate) -> String {
    render_view(view, today, None, true)
}

pub fn render_filtered(view: &View, today: NaiveDate, filter: &Filter) -> String {
    render_view(view, today, Some(filter), true)
}

fn render_view(
    view: &View,
    today: NaiveDate,
    filter: Option<&Filter>,
    result_numbers: bool,
) -> String {
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
    let mut result_number = 0;
    for day in &view.days {
        let is_active = Some(day.date) == view.active;
        let mut labels = vec![];
        if day.date == today {
            labels.push("today");
        }
        if is_active {
            labels.push("active — numbers are editable");
        } else if filter.is_some() {
            labels.push("history — letters scoped to this date");
        } else {
            labels.push("history — letters are browsing labels");
        }
        let _ = writeln!(output, "{} ({})", day.date, labels.join("; "));
        for status in Status::ALL {
            let items: Vec<_> = day
                .numbered()
                .into_iter()
                .enumerate()
                .filter(|(_, item)| item.status == status && filter.is_none_or(|f| f.matches(item)))
                .collect();
            if filter.is_some() && items.is_empty() {
                continue;
            }
            let _ = writeln!(output, "  {}", status.label());
            if items.is_empty() {
                output.push_str("    (empty)\n");
            }
            for (index, item) in items {
                let label = if is_active {
                    (index + 1).to_string()
                } else if filter.is_some() {
                    letters(index)
                } else {
                    let label = letters(historical_index);
                    historical_index += 1;
                    label
                };
                if result_numbers {
                    result_number += 1;
                    let _ = writeln!(output, "    [{result_number}] {label}. {}", item.title);
                } else {
                    let _ = writeln!(output, "    {label}. {}", item.title);
                }
            }
        }
        output.push('\n');
    }
    if result_numbers && result_number > 0 {
        output.push_str("Append `--repeat N` to this command to copy result [N] into active To do. Results are recomputed on each run.\n");
    }
    if filter.is_some() {
        output.push_str("Copy a match: `krk repeat NUMBER` (active), or `krk repeat --from YYYY-MM-DD LETTER` (history).\n");
    }
    output
}
