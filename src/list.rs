use crate::{storage, time};
use clap::{Parser, ValueEnum};
use color_eyre::eyre::Result;

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
            Filter::Yesterday => list_today(),
            Filter::Week => list_today(),
            Filter::Month => list_today(),
            Filter::Year => list_today(),
            Filter::All => list_today(),
            Filter::Since => list_today(),
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
    let rev_lines = storage::get_reverse_lines()?;

    let mut traversed_lines = Vec::new();
    for rev_line in rev_lines {
        if let Ok(line) = rev_line {
            let matched = line.starts_with('-');
            traversed_lines.push(line);
            if matched {
                break;
            }
        }
    }
    traversed_lines.push("Listing last day...\n".to_string());
    
    traversed_lines.reverse();
    Ok(traversed_lines.join("\n"))
}

fn list_today() -> Result<String> {
    let today = time::todays_date();
    let rev_lines = storage::get_reverse_lines()?;

    let mut traversed_lines = Vec::new();
    let mut last_date: Option<String> = None;
    for rev_line in rev_lines {
        if let Ok(line) = rev_line {
            let matched = line.starts_with('-');
            if matched {
                let date = line.trim_start_matches("- ");
                let xxx = date.to_string();
                last_date = Some(xxx);
                traversed_lines.push(date.to_string());
            } else {
                traversed_lines.push(line);
            }
            if matched {
                break;
            }
        }
    }

    traversed_lines.push("Listing today...\n".to_string());

    traversed_lines.reverse();
    if let Some(x) = last_date
        && x != today
    {}

    Ok(traversed_lines.join("\n"))
}

fn list_since(_: u32) -> Result<String> {
    // traversed_lines.push("Listing since...\n".to_string())

    // todo implement
    Ok("temp".to_string())
}
