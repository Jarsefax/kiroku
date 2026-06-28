use crate::{
    storage::{self, DATE_PREFIX},
    time,
};
use clap::Parser;
use color_eyre::eyre::{Ok, Result};
use colored::Colorize;

#[derive(Parser)]
#[command(visible_alias = "r")]
pub struct Cmd {}

pub fn execute() -> Result<String> {
    let (last_day, _) = storage::get_last_day()?;
    if last_day.is_empty() {
        return Ok("Nothing to rollover".to_string());
    }

    let mut rolled_over = Vec::new();
    for line in last_day {
        if line.starts_with(storage::TODO_PREFIX) {
            let ln = format!("{}\n", line);
            rolled_over.push(ln.normal());
        } else if line.starts_with(storage::DOING_PREFIX) {
            let todo = line.replacen(storage::DOING_PREFIX, storage::TODO_PREFIX, 1);
            let ln = format!("{}\n", todo);
            rolled_over.push(ln.normal());
        }
    }

    if rolled_over.is_empty() {
        return Ok("Nothing left to rollover".to_string());
    }

    let today = time::todays_date();
    let today_str = today.format(time::DATE_FORMAT).to_string();
    let ln = format!("{}{}\n", DATE_PREFIX, today_str);
    rolled_over.push(ln.blue());

    rolled_over.reverse();

    let text = rolled_over
        .iter()
        .fold(String::new(), |acc, num| format!("{}{}", acc, num));
    _ = storage::append_to_data_file(&text);

    Ok(text)
}
