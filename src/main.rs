mod tui;

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use kiroku::{
    config,
    listing::{self, View},
    storage::{Change, Day, Direction, Status, Store, parse_date},
    style::Style,
};
use std::{
    io::{self, IsTerminal, Write},
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
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        help = "Color command output: auto, always, or never"
    )]
    color: ColorMode,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Clone, Copy, ValueEnum)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    fn style(self, terminal: bool) -> Style {
        Style {
            enabled: match self {
                Self::Always => true,
                Self::Never => false,
                Self::Auto => {
                    terminal
                        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                        && std::env::var_os("TERM").is_none_or(|value| value != "dumb")
                }
            },
        }
    }
}

#[derive(clap::Args)]
struct AddTarget {
    #[arg(long, value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Add to this date instead of the active day")]
    date: Option<NaiveDate>,
}

#[derive(clap::Args)]
struct CopyResult {
    #[arg(
        long,
        value_name = "RESULT_NUMBER",
        help = "Copy the numbered result into To do; results are recomputed on this run"
    )]
    repeat: Option<usize>,
    #[arg(long, requires = "repeat", value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Copy to this date instead of the active day (requires --repeat)")]
    date: Option<NaiveDate>,
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
        #[arg(
            long,
            value_name = "TEXT",
            help = "Match title text, ignoring case; searches all history unless PERIOD is given"
        )]
        search: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Show only this status; searches all history unless PERIOD is given"
        )]
        status: Option<CliStatus>,
        #[command(flatten)]
        copy: CopyResult,
    },
    /// Search titles across all recorded days, or within a selected period
    Search {
        #[arg(value_name = "TEXT")]
        query: String,
        #[arg(
            value_name = "PERIOD",
            help = "today | yesterday | week | month | YYYY-MM-DD | FROM..TO"
        )]
        period: Option<String>,
        #[arg(long, value_enum)]
        status: Option<CliStatus>,
        #[command(flatten)]
        copy: CopyResult,
    },
    /// Add a title, or move a numbered active-day item into Doing
    Doing {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
        #[command(flatten)]
        target: AddTarget,
    },
    /// Add a title, or move a numbered active-day item into Done
    Done {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
        #[command(flatten)]
        target: AddTarget,
    },
    /// Add a title, or move a numbered active-day item back into To do
    Todo {
        #[arg(value_name = "TITLE_OR_NUMBER")]
        value: String,
        #[command(flatten)]
        target: AddTarget,
    },
    /// Copy an item into the active day, preserving its original entry
    Repeat {
        #[arg(
            value_name = "NUMBER_OR_LETTER",
            help = "Active-day number, or a letter from a single-date listing with --from"
        )]
        item: String,
        #[arg(long, value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Copy from another recorded day; letters are scoped to this date")]
        from: Option<chrono::NaiveDate>,
        #[arg(
            long,
            value_enum,
            default_value = "todo",
            help = "Status of the new copy"
        )]
        status: CliStatus,
        #[command(flatten)]
        target: AddTarget,
    },
    /// Rename an active-day item
    Edit { number: usize, title: String },
    /// Move an active-day item up or down within its section
    Reorder {
        number: usize,
        #[arg(value_enum)]
        direction: CliDirection,
        /// Number of places to move (stops at the section boundary)
        #[arg(long, default_value = "1", value_name = "N")]
        steps: std::num::NonZeroUsize,
    },
    /// Remove an active-day item
    Remove { number: usize },
    /// Print the resolved log directory
    Path,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliStatus {
    Todo,
    Doing,
    Done,
}

impl From<CliStatus> for Status {
    fn from(status: CliStatus) -> Self {
        match status {
            CliStatus::Todo => Self::Todo,
            CliStatus::Doing => Self::Doing,
            CliStatus::Done => Self::Done,
        }
    }
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

fn updated(day: Day, today: chrono::NaiveDate, style: Style) -> Result<()> {
    output(&listing::render_styled(
        &View {
            active: Some(day.date),
            days: vec![day],
            notice: "Updated active day; item numbers may have changed.".into(),
        },
        today,
        None,
        false,
        style,
    ))
}

fn updated_date(day: Day, style: Style) -> Result<()> {
    // A targeted historical addition must not advertise active-day numbers.
    let mut text = format!("{}\n\n", style.heading(&format!("Updated {}.", day.date)));
    for status in Status::ALL {
        text.push_str(&format!("  {}\n", style.status(status.label(), status)));
        let mut empty = true;
        for item in day.items.iter().filter(|item| item.status == status) {
            text.push_str(&format!("    - {}\n", item.title));
            empty = false;
        }
        if empty {
            text.push_str(&format!("    {}\n", style.muted("(empty)")));
        }
    }
    output(&text)
}

fn filtered_list(
    store: &Store,
    period: Option<&str>,
    query: &str,
    status: Option<CliStatus>,
    copy: CopyResult,
    today: chrono::NaiveDate,
    style: Style,
) -> Result<()> {
    let filter = listing::Filter::new(query, status.map(Into::into))?;
    let view = listing::select_filtered(store, period, today, &filter)?;
    show_or_repeat(store, view, Some(&filter), copy, today, style)
}

fn show_or_repeat(
    store: &Store,
    view: View,
    filter: Option<&listing::Filter>,
    copy: CopyResult,
    today: chrono::NaiveDate,
    style: Style,
) -> Result<()> {
    if let Some(result) = copy.repeat {
        let (source, number) = view.result(result, filter)?;
        if let Some(date) = copy.date {
            let day = store.repeat_on_date(
                date,
                Some(source.date),
                number,
                Status::Todo,
                Some(source),
            )?;
            output(&format!(
                "Repeated result {result} from {}: {}\n\n",
                style.heading(&source.date.to_string()),
                source.numbered()[number - 1].title
            ))?;
            return updated_date(day, style);
        }
        let active = store
            .active()?
            .context("No day has been started. Run `krk new-day` first")?;
        anyhow::ensure!(
            Some(active.date) == view.active,
            "The active day changed during the search or listing. Rerun the command"
        );
        let from = (source.date != active.date).then_some(source.date);
        let day = store.repeat(from, number, Status::Todo, Some(source), Some(&active))?;
        output(&format!(
            "Repeated result {result} from {}: {}\n\n",
            style.heading(&source.date.to_string()),
            source.numbered()[number - 1].title
        ))?;
        return updated(day, today, style);
    }
    output(&listing::render_styled(&view, today, filter, true, style))
}

fn run(args: Args, style: Style) -> Result<()> {
    let store = Store::new(config::resolve(args.data_dir.as_deref())?);
    let today = Local::now().date_naive();
    let (change, date) = match args.command {
        None => return tui::run(store),
        Some(Command::Path) => return output(&format!("{}\n", store.dir.display())),
        Some(Command::Search {
            query,
            period,
            status,
            copy,
        }) => {
            return filtered_list(
                &store,
                period.as_deref(),
                &query,
                status,
                copy,
                today,
                style,
            );
        }
        Some(Command::List {
            period,
            search,
            status,
            copy,
        }) => {
            if search.is_some() || status.is_some() {
                return filtered_list(
                    &store,
                    period.as_deref(),
                    search.as_deref().unwrap_or(""),
                    status,
                    copy,
                    today,
                    style,
                );
            }
            return show_or_repeat(
                &store,
                listing::select(&store, period.as_deref(), today)?,
                None,
                copy,
                today,
                style,
            );
        }
        Some(Command::NewDay) => {
            let created = store.new_day(today)?;
            output(&format!(
                "{}: {}\n\n",
                style.heading(&today.to_string()),
                if created {
                    "day started"
                } else {
                    "day already started; no changes made"
                }
            ))?;
            return output(&listing::render_styled(
                &listing::select(&store, None, today)?,
                today,
                None,
                false,
                style,
            ));
        }
        Some(Command::Doing { value, target }) => {
            (status_change(value, Status::Doing)?, target.date)
        }
        Some(Command::Done { value, target }) => (status_change(value, Status::Done)?, target.date),
        Some(Command::Todo { value, target }) => (status_change(value, Status::Todo)?, target.date),
        Some(Command::Repeat {
            item,
            from,
            status,
            target,
        }) => {
            let number = if from.is_some() {
                listing::letter_number(&item)?
            } else {
                anyhow::ensure!(
                    !item.is_empty() && item.bytes().all(|b| b.is_ascii_digit()),
                    "Use an active-day number, or --from YYYY-MM-DD with a historical letter"
                );
                item.parse().context("Item number is too large")?
            };
            if let Some(date) = target.date {
                return updated_date(
                    store.repeat_on_date(date, from, number, status.into(), None)?,
                    style,
                );
            }
            return updated(
                store.repeat(from, number, status.into(), None, None)?,
                today,
                style,
            );
        }
        Some(Command::Edit { number, title }) => (Change::Edit(number, title), None),
        Some(Command::Reorder {
            number,
            direction,
            steps,
        }) => (
            Change::Reorder(
                number,
                match direction {
                    CliDirection::Up => Direction::Up,
                    CliDirection::Down => Direction::Down,
                },
                steps.get(),
            ),
            None,
        ),
        Some(Command::Remove { number }) => (Change::Remove(number), None),
    };
    if let Some(date) = date {
        let Change::Add(title, status) = change else {
            anyhow::bail!(
                "--date adds titles or copies items; numbered moves still apply to the active day"
            );
        };
        return updated_date(store.add_on_date(date, &title, status)?, style);
    }
    updated(store.change(change, None)?, today, style)
}

fn main() {
    let args = Args::parse();
    let stdout_style = args.color.style(io::stdout().is_terminal());
    let stderr_style = args.color.style(io::stderr().is_terminal());
    if let Err(error) = run(args, stdout_style) {
        eprintln!("{} {error:#}", stderr_style.error("krk:"));
        std::process::exit(1);
    }
}
