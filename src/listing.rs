use crate::storage::{Day, Item, Status, Store, parse_date};
use crate::style::Style;
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
    let active = store.active_on(today)?.map(|day| day.date);
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
        format!("{count} matching {noun}.")
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
    let active_day = store.active_on(today)?;
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
            "No recorded days on or before today. Today ({today}) has not been started. Run `krk new-day` to start."
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

/// Right-align a bracketed item identifier before applying terminal styling.
pub fn item_label(label: &str, width: usize) -> String {
    format!("{:>width$}", format!("[{label}]"), width = width + 2)
}

pub fn render(view: &View, today: NaiveDate) -> String {
    render_styled(view, today, None, false, Style::default())
}

pub fn render_list(view: &View, today: NaiveDate) -> String {
    render_styled(view, today, None, true, Style::default())
}

pub fn render_filtered(view: &View, today: NaiveDate, filter: &Filter) -> String {
    render_styled(view, today, Some(filter), true, Style::default())
}

pub fn render_styled(
    view: &View,
    today: NaiveDate,
    filter: Option<&Filter>,
    result_numbers: bool,
    style: Style,
) -> String {
    let mut output = String::new();
    if !view.notice.is_empty() {
        let _ = writeln!(output, "{}\n", style.muted(&view.notice));
    }
    if view.days.is_empty() {
        if view.notice.is_empty() {
            let _ = writeln!(
                output,
                "{}",
                style.muted("No recorded days. Run `krk new-day` to start.")
            );
        }
        return output;
    }
    let label_width = if result_numbers {
        view.days
            .iter()
            .flat_map(|day| &day.items)
            .filter(|item| filter.is_none_or(|f| f.matches(item)))
            .count()
            .to_string()
            .len()
    } else {
        let historical_count: usize = view
            .days
            .iter()
            .filter(|day| Some(day.date) != view.active)
            .map(|day| day.items.len())
            .sum();
        let numeric_width = view
            .days
            .iter()
            .filter(|day| Some(day.date) == view.active)
            .map(|day| day.items.len().to_string().len())
            .max()
            .unwrap_or(1);
        numeric_width.max(letters(historical_count.saturating_sub(1)).len())
    };
    let editable_numbers =
        filter.is_none() && view.days.iter().all(|day| Some(day.date) == view.active);
    let mut historical_index = 0;
    let mut result_number = 0;
    for day in &view.days {
        let is_active = Some(day.date) == view.active;
        let mut labels = vec![];
        if day.date == today {
            labels.push("today");
        }
        if is_active && (!result_numbers || editable_numbers) {
            labels.push("active — numbers are editable");
        } else if is_active {
            labels.push("active — result numbers are for copying");
        } else if result_numbers && day.date > today {
            labels.push("future — result numbers are for copying");
        } else if result_numbers {
            labels.push("history — result numbers are for copying");
        } else if day.date > today && filter.is_some() {
            labels.push("future — letters scoped to this date");
        } else if day.date > today {
            labels.push("future — letters are browsing labels");
        } else if filter.is_some() {
            labels.push("history — letters scoped to this date");
        } else {
            labels.push("history — letters are browsing labels");
        }
        let _ = writeln!(
            output,
            "{} {}",
            style.heading(&day.date.to_string()),
            style.muted(&format!("({})", labels.join("; ")))
        );
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
            let _ = writeln!(output, "  {}", style.status(status.label(), status));
            if items.is_empty() {
                let _ = writeln!(output, "    {}", style.muted("(empty)"));
            }
            for (index, item) in items {
                let label = if result_numbers {
                    result_number += 1;
                    result_number.to_string()
                } else if is_active {
                    (index + 1).to_string()
                } else if filter.is_some() {
                    letters(index)
                } else {
                    let label = letters(historical_index);
                    historical_index += 1;
                    label
                };
                let _ = writeln!(
                    output,
                    "    {} {}",
                    style.status(&item_label(&label, label_width), status),
                    item.title
                );
            }
        }
        output.push('\n');
    }
    if result_numbers && result_number > 0 {
        let _ = writeln!(output, "{}", style.muted("Append `--repeat N` to this command to copy result [N] into active To do. Results are recomputed on each run."));
    }
    if result_numbers && result_number > 0 && !editable_numbers {
        let _ = writeln!(
            output,
            "{}",
            style
                .muted("For active-day edits, run `krk list` without filters to get item numbers.")
        );
    }
    output
}
