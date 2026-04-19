use crate::storage;
use clap::{Parser, ValueEnum};
use color_eyre::eyre::Result;
use rev_lines::RevLines;
use std::fs::File;

#[derive(Parser)]
#[command(visible_aliases = ["l", "ls"])]
pub struct Cmd {
    /// How long back to list
    #[arg(value_enum, short, long, alias = "fltr")]
    filter: Option<Filter>,

    /// How many days back
    #[arg(short, long)]
    days: Option<u32>,

    /// How many weeks back
    #[arg(short, long, aliases = ["wks", "weks"])]
    weeks: Option<u32>,

    /// How many months back
    #[arg(short, long)]
    months: Option<u32>,

    /// Expected number of days in a week
    #[arg(long, visible_alias = "dpw", hide_short_help = true)]
    days_per_week: Option<u32>,
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

    /// Items since
    #[value(aliases = ["s", "snc"])]
    Since,

    /// All items
    #[value(aliases = ["a", "al"])]
    All,
}

pub fn execute(_cmd: Cmd) -> Result<String> {
    let path = storage::get_data_file_path_buf()?;

    let file = File::open(path)?;
    let rev_lines = RevLines::new(file);

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
    traversed_lines.reverse();

    Ok(traversed_lines.join("\n"))
}
