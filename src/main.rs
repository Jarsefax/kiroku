mod tui;

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use kiroku::{
    config,
    listing::{self, View},
    storage::{Change, Status, Store},
};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    name = "krk",
    version,
    about = "Kiroku — a daily work log in monthly Markdown files"
)]
struct Args {
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Override the log directory"
    )]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Explicitly start today and carry unfinished items into To do
    NewDay,
    /// List today, a relative period, a date, or an inclusive FROM..TO range
    List {
        #[arg(
            value_name = "PERIOD",
            help = "today | yesterday | week | month | YYYY-MM-DD | FROM..TO"
        )]
        period: Option<String>,
    },
    /// Add a title, or move a numbered active-day item into Doing
    Doing {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
    },
    /// Add a title, or move a numbered active-day item into Done
    Done {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
    },
    /// Add a title, or move a numbered active-day item back into To do
    Todo {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
    },
    /// Rename an active-day item
    Edit { number: usize, title: String },
    /// Remove an active-day item
    Remove { number: usize },
    /// Print the resolved log directory
    Path,
}

fn output(text: &str) -> Result<()> {
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.context("Cannot write output"),
    }
}

fn status_change(value: String, status: Status) -> Result<Change> {
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(Change::Move(
            value.parse().context("Item number is too large")?,
            status,
        ))
    } else {
        Ok(Change::Add(value, status))
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let store = Store::new(config::resolve(args.data_dir.as_deref())?);
    let today = Local::now().date_naive();
    let change = match args.command {
        None => return tui::run(store),
        Some(Command::Path) => return output(&format!("{}\n", store.dir.display())),
        Some(Command::List { period }) => {
            return output(&listing::render(
                &listing::select(&store, period.as_deref(), today)?,
                today,
            ));
        }
        Some(Command::NewDay) => {
            let created = store.new_day(today)?;
            output(&format!(
                "{today}: {}\n\n",
                if created {
                    "day started"
                } else {
                    "day already started; no changes made"
                }
            ))?;
            return output(&listing::render(
                &listing::select(&store, None, today)?,
                today,
            ));
        }
        Some(Command::Doing { value }) => status_change(value, Status::Doing)?,
        Some(Command::Done { value }) => status_change(value, Status::Done)?,
        Some(Command::Todo { value }) => status_change(value, Status::Todo)?,
        Some(Command::Edit { number, title }) => Change::Edit(number, title),
        Some(Command::Remove { number }) => Change::Remove(number),
    };
    let day = store.change(change, None)?;
    output(&listing::render(
        &View {
            active: Some(day.date),
            days: vec![day],
            notice: "Updated active day; item numbers may have changed.".into(),
        },
        today,
    ))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("krk: {error:#}");
        std::process::exit(1);
    }
}
