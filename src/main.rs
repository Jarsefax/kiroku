mod tui;

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use kiroku::{
    config,
    listing::{self, View},
    selection::Selection,
    storage::{Change, Day, Direction, ItemChange, Status, Store, parse_date},
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
        short = 'D',
        long,
        global = true,
        value_name = "PATH",
        help = "Override the log directory"
    )]
    data_dir: Option<PathBuf>,
    #[arg(
        short,
        long,
        global = true,
        value_enum,
        default_value = "auto",
        help = "Color command output: auto, always, or never"
    )]
    color: ColorMode,
    #[arg(
        long,
        global = true,
        help = "Omit the separator before long terminal output"
    )]
    no_separator: bool,
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
    #[arg(
        short,
        long,
        value_name = "N",
        help = "Insert at this one-based position within the destination section (default: append)"
    )]
    position: Option<std::num::NonZeroUsize>,
    #[arg(short, long, value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Use this destination date instead of the active day")]
    date: Option<NaiveDate>,
}

#[derive(clap::Args)]
struct StatusTarget {
    #[command(flatten)]
    target: AddTarget,
    #[arg(short, long, requires = "date", value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Move a date-scoped letter from this recorded day (requires --date)")]
    from: Option<NaiveDate>,
}

#[derive(clap::Args)]
struct ResultAction {
    #[arg(
        short,
        long,
        requires = "repeat",
        value_name = "N",
        help = "Insert the copy at this one-based position in To do (requires --repeat)"
    )]
    position: Option<std::num::NonZeroUsize>,
    #[arg(
        short,
        long,
        value_name = "RESULTS",
        help = "Copy results into To do (for example 1-3,5); results are recomputed on this run"
    )]
    repeat: Option<Selection>,
    #[arg(
        short = 'x',
        long,
        conflicts_with_all = ["repeat", "date", "position"],
        value_name = "RESULTS",
        help = "Remove results from their recorded days (for example 1-3,5); results are recomputed on this run"
    )]
    remove: Option<Selection>,
    #[arg(short, long, requires = "repeat", value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Copy to this date instead of the active day (requires --repeat)")]
    date: Option<NaiveDate>,
}

#[derive(Subcommand)]
enum Command {
    /// Explicitly start today and move unfinished items into its To do section
    #[command(visible_alias = "n")]
    NewDay,
    /// List today, a relative period, a date, or an inclusive FROM..TO range
    #[command(visible_alias = "l")]
    List {
        #[arg(
            value_name = "PERIOD",
            help = "today | yesterday | week | month | YYYY-MM-DD | FROM..TO"
        )]
        period: Option<String>,
        #[arg(
            short = 'S',
            long,
            value_name = "TEXT",
            help = "Match title text, ignoring case; searches all history unless PERIOD is given"
        )]
        search: Option<String>,
        #[arg(
            short,
            long,
            value_enum,
            help = "Show only this status; searches all history unless PERIOD is given"
        )]
        status: Option<CliStatus>,
        #[command(flatten)]
        action: ResultAction,
    },
    /// Search titles across all recorded days, or within a selected period
    #[command(visible_alias = "s")]
    Search {
        #[arg(value_name = "TEXT")]
        query: String,
        #[arg(
            value_name = "PERIOD",
            help = "today | yesterday | week | month | YYYY-MM-DD | FROM..TO"
        )]
        period: Option<String>,
        #[arg(short, long, value_enum)]
        status: Option<CliStatus>,
        #[command(flatten)]
        action: ResultAction,
    },
    /// Add a title, or move an item into Doing, optionally on another date
    #[command(visible_alias = "do")]
    Doing {
        #[arg(
            value_name = "TITLE_OR_REFERENCE",
            help = "Add a title, or select items (1-3,5; A-C,E with --from)"
        )]
        value: String,
        #[command(flatten)]
        target: StatusTarget,
    },
    /// Add a title, or move an item into Done, optionally on another date
    #[command(visible_alias = "d")]
    Done {
        #[arg(
            value_name = "TITLE_OR_REFERENCE",
            help = "Add a title, or select items (1-3,5; A-C,E with --from)"
        )]
        value: String,
        #[command(flatten)]
        target: StatusTarget,
    },
    /// Add a title, or move an item into To do, optionally on another date
    #[command(visible_alias = "t")]
    Todo {
        #[arg(
            value_name = "TITLE_OR_REFERENCE",
            help = "Add a title, or select items (1-3,5; A-C,E with --from)"
        )]
        value: String,
        #[command(flatten)]
        target: StatusTarget,
    },
    /// Copy selected items into the active day, preserving their original entries
    #[command(visible_alias = "r")]
    Repeat {
        #[arg(
            value_name = "ITEMS_OR_LETTERS",
            help = "Active-day numbers (1-3,5), or date-scoped letters with --from (A-C,E)"
        )]
        item: String,
        #[arg(short, long, value_name = "YYYY-MM-DD", value_parser = parse_date, help = "Copy from another recorded day; letters are scoped to this date")]
        from: Option<chrono::NaiveDate>,
        #[arg(
            short,
            long,
            value_enum,
            default_value = "todo",
            help = "Status of the new copy"
        )]
        status: CliStatus,
        #[command(flatten)]
        target: AddTarget,
    },
    /// Rename selected active-day items to the supplied title
    #[command(visible_alias = "e")]
    Edit {
        #[arg(
            value_name = "ITEMS",
            help = "Item numbers, ranges, or lists (for example 1-3,5)"
        )]
        selection: Selection,
        title: String,
    },
    /// Move selected active-day items up or down within their sections
    #[command(visible_alias = "m")]
    Move {
        #[arg(
            value_name = "ITEMS",
            help = "Item numbers, ranges, or lists (for example 1-3,5)"
        )]
        selection: Selection,
        #[arg(value_enum)]
        direction: CliDirection,
        /// Number of places to move (stops at the section boundary)
        #[arg(short = 'n', long, default_value = "1", value_name = "N")]
        steps: std::num::NonZeroUsize,
    },
    /// Remove selected active-day items
    #[command(visible_alias = "x")]
    Remove {
        #[arg(
            value_name = "ITEMS",
            help = "Item numbers, ranges, or lists (for example 1-3,5)"
        )]
        selection: Selection,
    },
    /// Print the resolved log directory
    #[command(visible_alias = "p")]
    Path,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::NewDay => "new-day",
            Self::List { .. } => "list",
            Self::Search { .. } => "search",
            Self::Doing { .. } => "doing",
            Self::Done { .. } => "done",
            Self::Todo { .. } => "todo",
            Self::Repeat { .. } => "repeat",
            Self::Edit { .. } => "edit",
            Self::Move { .. } => "move",
            Self::Remove { .. } => "remove",
            Self::Path => "path",
        }
    }
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

struct CommandOutput {
    style: Style,
    separator: Option<&'static str>,
    pending: String,
}

impl CommandOutput {
    fn write(&mut self, text: &str) -> Result<()> {
        let Some(command) = self.separator else {
            return output(text);
        };
        // Hold short preambles until we know whether the whole output needs a boundary.
        self.pending.push_str(text);
        if self
            .pending
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(3)
            .count()
            < 3
        {
            return Ok(());
        }
        self.separator = None;
        let heading = format!("── krk {command} ");
        let width = crossterm::terminal::size()
            .ok()
            .map(|(width, _)| usize::from(width))
            .filter(|&width| width > 0)
            .unwrap_or(40)
            .min(60);
        let rule = "─".repeat(width.saturating_sub(heading.chars().count()));
        output(&format!(
            "\n{}\n",
            self.style.muted(&format!("{heading}{rule}"))
        ))?;
        self.finish()
    }

    fn finish(&mut self) -> Result<()> {
        output(&std::mem::take(&mut self.pending))
    }
}

fn source_day(store: &Store, from: Option<NaiveDate>) -> Result<Day> {
    if let Some(date) = from {
        store
            .days(Some((date, date)))?
            .pop()
            .with_context(|| format!("No recorded day for {date}"))
    } else {
        store
            .active()?
            .context("No day has been started. Run `krk new-day` first")
    }
}

fn parse_references(value: &str, from: Option<NaiveDate>) -> Result<Selection> {
    if from.is_some() {
        Selection::letters(value)
    } else {
        value.parse()
    }
}

fn change_selected(store: &Store, selection: Selection, change: ItemChange) -> Result<Day> {
    let source = source_day(store, None)?;
    let numbers = selection.resolve(source.items.len())?;
    store.change_items(&numbers, change, Some(&source))
}

fn status_command(
    store: &Store,
    value: String,
    status: Status,
    target: StatusTarget,
    today: NaiveDate,
    out: &mut CommandOutput,
) -> Result<()> {
    let position = target.target.position.map(|position| position.get());
    let day = if target.from.is_some() || Selection::looks_numeric(&value) {
        let selection = parse_references(&value, target.from)?;
        let source = source_day(store, target.from)?;
        let numbers = selection.resolve(source.items.len())?;
        if let Some(date) = target.target.date {
            store.move_items_on_date_at(
                date,
                target.from,
                &numbers,
                status,
                Some(&source),
                position,
            )?
        } else {
            store.change_items(&numbers, ItemChange::Move(status, position), Some(&source))?
        }
    } else if let Some(date) = target.target.date {
        store.add_on_date_at(date, &value, status, position)?
    } else {
        let change = match position {
            Some(position) => Change::AddAt(value, status, position),
            None => Change::Add(value, status),
        };
        store.change(change, None)?
    };
    if target.target.date.is_some() {
        updated_date(day, out)
    } else {
        updated(day, today, out)
    }
}

fn updated(day: Day, today: chrono::NaiveDate, out: &mut CommandOutput) -> Result<()> {
    out.write(&listing::render_styled(
        &View {
            active: Some(day.date),
            days: vec![day],
            notice: "Updated active day; item numbers may have changed.".into(),
        },
        today,
        None,
        false,
        out.style,
    ))
}

fn updated_date(day: Day, out: &mut CommandOutput) -> Result<()> {
    let style = out.style;
    // A targeted update must not advertise historical items as active-day numbers.
    let mut text = format!(
        "{}\n\n",
        style.heading(&format!("Updated {}.", listing::format_date(day.date)))
    );
    for status in Status::ALL {
        let mut items = day
            .items
            .iter()
            .filter(|item| item.status == status)
            .peekable();
        if items.peek().is_none() {
            continue;
        }
        text.push_str(&format!("  {}\n", style.status(status.label(), status)));
        for item in items {
            text.push_str(&format!("    - {}\n", item.title));
        }
    }
    out.write(&text)
}

fn filtered_list(
    store: &Store,
    period: Option<&str>,
    query: &str,
    status: Option<CliStatus>,
    action: ResultAction,
    today: chrono::NaiveDate,
    out: &mut CommandOutput,
) -> Result<()> {
    let filter = listing::Filter::new(query, status.map(Into::into))?;
    show_or_change(store, period, Some(&filter), action, today, out)
}

fn show_or_change(
    store: &Store,
    period: Option<&str>,
    filter: Option<&listing::Filter>,
    action: ResultAction,
    today: chrono::NaiveDate,
    out: &mut CommandOutput,
) -> Result<()> {
    let style = out.style;
    let select = || match filter {
        Some(filter) => listing::select_filtered(store, period, today, filter),
        None => listing::select(store, period, today),
    };
    let view = select()?;
    if let Some(selection) = action.remove.as_ref().or(action.repeat.as_ref()) {
        let available = view
            .days
            .iter()
            .flat_map(|day| &day.items)
            .filter(|item| filter.is_none_or(|filter| filter.matches(item)))
            .count();
        let results = selection
            .resolve(available)?
            .into_iter()
            .map(|result| {
                view.result(result, filter)
                    .map(|(source, number)| (result, source, number))
            })
            .collect::<Result<Vec<_>>>()?;
        let selected = results
            .iter()
            .map(|&(_, source, number)| (source, number))
            .collect::<Vec<_>>();
        if action.remove.is_some() {
            store.remove_results(&selected)?;
            for &(result, source, number) in &results {
                out.write(&format!(
                    "Removed result {result} from {}: {}\n",
                    style.heading(&listing::format_date(source.date)),
                    source.numbered()[number - 1].title
                ))?;
            }
            out.write("\n")?;
            return out.write(&listing::render_styled(
                &select()?,
                today,
                filter,
                true,
                style,
            ));
        }
        let active = if action.date.is_none() {
            let active = source_day(store, None)?;
            anyhow::ensure!(
                Some(active.date) == view.active,
                "The active day changed during the search or listing. Rerun the command"
            );
            Some(active)
        } else {
            None
        };
        let day = store.repeat_results(
            action.date,
            &selected,
            Status::Todo,
            active.as_ref(),
            action.position.map(|position| position.get()),
        )?;
        for &(result, source, number) in &results {
            out.write(&format!(
                "Repeated result {result} from {}: {}\n",
                style.heading(&listing::format_date(source.date)),
                source.numbered()[number - 1].title
            ))?;
        }
        out.write("\n")?;
        return if action.date.is_some() {
            updated_date(day, out)
        } else {
            updated(day, today, out)
        };
    }
    out.write(&listing::render_styled(&view, today, filter, true, style))
}

fn run(args: Args, style: Style) -> Result<()> {
    let separator = args
        .command
        .as_ref()
        .filter(|command| !matches!(command, Command::Path))
        .filter(|_| !args.no_separator && io::stdout().is_terminal())
        .map(Command::name);
    let mut out = CommandOutput {
        style,
        separator,
        pending: String::new(),
    };
    let result = run_command(args, &mut out);
    // Retain any successful preamble even if a later operation fails.
    out.finish()?;
    result
}

fn run_command(args: Args, out: &mut CommandOutput) -> Result<()> {
    let style = out.style;
    let store = Store::new(config::resolve(args.data_dir.as_deref())?);
    let today = Local::now().date_naive();
    match args.command {
        None => tui::run(store),
        Some(Command::Path) => out.write(&format!("{}\n", store.dir.display())),
        Some(Command::Search {
            query,
            period,
            status,
            action,
        }) => filtered_list(
            &store,
            period.as_deref(),
            &query,
            status,
            action,
            today,
            out,
        ),
        Some(Command::List {
            period,
            search,
            status,
            action,
        }) => {
            if search.is_some() || status.is_some() {
                return filtered_list(
                    &store,
                    period.as_deref(),
                    search.as_deref().unwrap_or(""),
                    status,
                    action,
                    today,
                    out,
                );
            }
            show_or_change(&store, period.as_deref(), None, action, today, out)
        }
        Some(Command::NewDay) => {
            let created = store.new_day(today)?;
            out.write(&format!(
                "{}: {}\n\n",
                style.heading(&listing::format_date(today)),
                if created {
                    "day started"
                } else {
                    "day already started; no changes made"
                }
            ))?;
            out.write(&listing::render_styled(
                &listing::select(&store, None, today)?,
                today,
                None,
                false,
                style,
            ))
        }
        Some(Command::Doing { value, target }) => {
            status_command(&store, value, Status::Doing, target, today, out)
        }
        Some(Command::Done { value, target }) => {
            status_command(&store, value, Status::Done, target, today, out)
        }
        Some(Command::Todo { value, target }) => {
            status_command(&store, value, Status::Todo, target, today, out)
        }
        Some(Command::Repeat {
            item,
            from,
            status,
            target,
        }) => {
            let selection = parse_references(&item, from)?;
            let source = source_day(&store, from)?;
            let numbers = selection.resolve(source.items.len())?;
            let active = if target.date.is_none() {
                let active = source_day(&store, None)?;
                anyhow::ensure!(
                    from.is_none() || source.date != active.date,
                    "--from must select a different recorded day; use active-day numbers otherwise"
                );
                Some(active)
            } else {
                None
            };
            let selected = numbers
                .iter()
                .map(|&number| (&source, number))
                .collect::<Vec<_>>();
            let day = store.repeat_results(
                target.date,
                &selected,
                status.into(),
                active.as_ref(),
                target.position.map(|position| position.get()),
            )?;
            if target.date.is_some() {
                updated_date(day, out)
            } else {
                updated(day, today, out)
            }
        }
        Some(Command::Edit { selection, title }) => updated(
            change_selected(&store, selection, ItemChange::Edit(title))?,
            today,
            out,
        ),
        Some(Command::Move {
            selection,
            direction,
            steps,
        }) => updated(
            change_selected(
                &store,
                selection,
                ItemChange::Reorder(
                    match direction {
                        CliDirection::Up => Direction::Up,
                        CliDirection::Down => Direction::Down,
                    },
                    steps.get(),
                ),
            )?,
            today,
            out,
        ),
        Some(Command::Remove { selection }) => updated(
            change_selected(&store, selection, ItemChange::Remove)?,
            today,
            out,
        ),
    }
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
