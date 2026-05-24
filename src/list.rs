use crate::{storage, time};
use chrono::NaiveDate;
use clap::{Parser, ValueEnum};
use color_eyre::eyre::Result;
use colored::{ColoredString, Colorize};
use itertools::Itertools;

#[derive(Parser)]
#[command(visible_aliases = ["l", "ls"])]
pub struct Cmd {
    /// How long back to list
    #[arg(value_enum, short, long, alias = "fltr")]
    filter: Option<Filter>,

    /// How many days back
    #[arg(short, long, default_value_t = 0)]
    days: u32,

    /// How many weeks back
    #[arg(short, long, aliases = ["wks", "weks"], default_value_t = 0)]
    weeks: u32,

    /// How many months back
    #[arg(short, long, default_value_t = 0)]
    months: u32,

    /// How many years back
    #[arg(short, long, default_value_t = 0)]
    years: u32,

    /// Expected number of days in a week
    #[arg(
        long,
        visible_alias = "dpw",
        default_value_t = 0,
        hide_short_help = true
    )]
    days_per_week: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Filter {
    /// Items from today
    #[value(aliases = ["t", "td", "tday"])]
    Today,

    /// Items from yesterday
    #[value(aliases = ["y", "yd", "yday"])]
    Yesterday,

    /// Items from this week
    #[value(aliases = ["w", "wk"])]
    Week,

    /// Items from this month
    #[value(alias = "m")]
    Month,

    /// Items from this year
    #[value(alias = "y")]
    Year,

    /// Items since
    #[value(aliases = ["s", "snc"])]
    Since,

    /// All items
    #[value(aliases = ["a", "al"])]
    All,
}

pub fn execute(cmd: Cmd) -> Result<String> {
    if let Some(filter) = cmd.filter {
        match filter {
            Filter::Today => list_today(),
            Filter::Yesterday => list_yesterday(),
            Filter::Week => Ok("listing last week is not implemented yet".to_string()),
            Filter::Month => Ok("listing last month is not implemented yet".to_string()),
            Filter::Year => Ok("listing last year is not implemented yet".to_string()),
            Filter::All => Ok("listing all is not implemented yet".to_string()),
            Filter::Since => Ok("listing since a date is not implemented yet".to_string()),
        }
    } else {
        let mut days = cmd.days + (cmd.weeks * 7);
        let months = cmd.months;
        let years = cmd.years;
        if days > 0 || months > 0 || years > 0 {
            // TODO: look at which months and years
            days = days + (months * 30) + (years * 365);
            list_since(days)
        } else {
            list_last_day()
        }
    }
}

fn list_last_day() -> Result<String> {
    let (mut traversed_lines, _) = get_last_day()?;
    traversed_lines.push("Listing last day...\n".normal());

    traversed_lines.reverse();
    Ok(traversed_lines.iter().join("\n"))
}

fn get_last_day() -> Result<(Vec<ColoredString>, Option<String>)> {
    let mut last_date: Option<String> = None;

    let rev_lines = storage::get_reverse_lines()?;

    let mut traversed_lines = Vec::new();
    for line in rev_lines.flatten() {
        let matched = line.starts_with('-');
        if matched {
            let date = line.trim_start_matches("- ");
            let date_str = date.to_string();
            last_date = Some(date_str);

            let colored_line = line.blue();
            traversed_lines.push(colored_line);
            break;
        } else {
            let colored_line = line.normal();
            traversed_lines.push(colored_line);
        }
    }

    Ok((traversed_lines, last_date))
}

fn list_today() -> Result<String> {
    let today = time::todays_date();
    let (mut traversed_lines, last_date) = get_last_day()?;

    if let Some(date) = last_date
        && date != today.to_string()
    {
        let colored_line = "(No registrations today)\n".yellow();
        traversed_lines.push(colored_line);
    }
    traversed_lines.push("Listing today...\n".normal());

    traversed_lines.reverse();
    Ok(traversed_lines.iter().join("\n"))
}

fn list_yesterday() -> Result<String> {
    let today = time::todays_date();
    let yesterday = time::yesterdays_date();
    let rev_lines = storage::get_reverse_lines()?;

    let mut found = false;
    let mut traversed_lines = Vec::new();
    for line in rev_lines.flatten() {
        let matched = line.starts_with('-');
        if matched {
            let date_str = line.trim_start_matches("- ");
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d");
            if date == Ok(today) {
                let today_str = today.format("%Y-%m-%d").to_string();
                traversed_lines.push(today_str);
            } else if date == Ok(yesterday) {
                found = true;
                let yesterday_str = yesterday.to_string();
                traversed_lines.push(yesterday_str);
                break;
            } else {
                break;
            }
        } else {
            traversed_lines.push(line);
        }
    }
    if found {
        traversed_lines.reverse();
        Ok(traversed_lines.join("\n"))
    } else {
        let nothing = Colorize::red("Nothing recorded yesterday");
        Ok(nothing.to_string())
    }
}

fn list_since(_: u32) -> Result<String> {
    // traversed_lines.push("Listing since...\n".to_string())

    // todo implement
    Ok("temp".to_string())
}
