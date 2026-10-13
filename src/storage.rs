use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, Local, NaiveDate};
use fs2::FileExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    path::PathBuf,
};

// Independent of the application release version in Cargo.toml.
const STORAGE_FORMAT_VERSION: u32 = 1;

fn parse_format_version(comment: &str) -> Result<u32> {
    let invalid = || "Invalid kiroku-format header; expected <!-- kiroku-format: 1 -->";
    let value = comment
        .strip_prefix("kiroku-format:")
        .and_then(|text| text.strip_suffix("-->"))
        .map(str::trim)
        .with_context(invalid)?;
    ensure!(
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        "{}",
        invalid()
    );
    let version: u32 = value.parse().with_context(invalid)?;
    ensure!(version > 0, "{}", invalid());
    ensure!(
        version == STORAGE_FORMAT_VERSION,
        "Storage format {version} is unsupported; this app supports format {STORAGE_FORMAT_VERSION}. Upgrade Kiroku before reading or changing this file"
    );
    Ok(version)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Todo,
    Doing,
    Done,
}

impl Status {
    pub const ALL: [Self; 3] = [Self::Todo, Self::Doing, Self::Done];
    pub fn label(self) -> &'static str {
        match self {
            Self::Todo => "To do",
            Self::Doing => "Doing",
            Self::Done => "Done",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Todo => 0,
            Self::Doing => 1,
            Self::Done => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub title: String,
    pub status: Status,
    line: usize,
}

#[derive(Clone, Debug)]
pub struct Day {
    pub date: NaiveDate,
    pub items: Vec<Item>,
    headings: [usize; 3],
    start: usize,
    end: usize,
}

impl Day {
    pub fn numbered(&self) -> Vec<&Item> {
        Status::ALL
            .into_iter()
            .flat_map(|status| self.items.iter().filter(move |i| i.status == status))
            .collect()
    }

    fn matches(&self, expected: &Day) -> bool {
        self.date == expected.date
            && self
                .items
                .iter()
                .map(|i| (&i.title, i.status))
                .eq(expected.items.iter().map(|i| (&i.title, i.status)))
    }
}

fn selected_items(day: &Day, numbers: &[usize]) -> Result<Vec<Item>> {
    ensure!(!numbers.is_empty(), "Select at least one item");
    let numbered = day.numbered();
    numbers
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|number| {
            numbered
                .get(number.wrapping_sub(1))
                .copied()
                .cloned()
                .with_context(|| format!("No source item {number} on {}", day.date))
        })
        .collect()
}

pub fn parse_date(text: &str) -> Result<NaiveDate> {
    let date = NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .with_context(|| format!("Invalid date {text:?}; use YYYY-MM-DD"))?;
    ensure!(
        date.format("%Y-%m-%d").to_string() == text && (1..=9999).contains(&date.year()),
        "Invalid date {text:?}; use YYYY-MM-DD"
    );
    Ok(date)
}

pub fn validate_title(title: &str) -> Result<&str> {
    ensure!(
        !title.chars().any(char::is_control),
        "Titles must be a single line without control characters"
    );
    let title = title.trim();
    ensure!(!title.is_empty(), "An item needs a nonempty title");
    Ok(title)
}

struct Document {
    path: PathBuf,
    original: Option<String>,
    lines: Vec<String>,
    days: Vec<Day>,
    newline: &'static str,
    has_format_header: bool,
}

impl Document {
    fn read_or_empty(path: PathBuf) -> Result<Self> {
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file(),
                    "{} is not a regular log file",
                    path.display()
                );
                Self::read(path)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::parse(path, None),
            Err(error) => Err(error).with_context(|| format!("Cannot inspect {}", path.display())),
        }
    }

    fn read(path: PathBuf) -> Result<Self> {
        let text =
            fs::read_to_string(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        Self::parse(path, Some(text))
    }

    fn parse(path: PathBuf, original: Option<String>) -> Result<Self> {
        let text = original.as_deref().unwrap_or("");
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
        let mut days: Vec<Day> = vec![];
        let mut status = None;
        let mut in_comment = false;
        let mut has_format_header = false;
        for (index, raw) in lines.iter().enumerate() {
            let line = raw.trim_end_matches(['\r', '\n']);
            let trimmed = line.trim();
            let invalid = || {
                format!(
                    "{}:{}: unrecognized content; use date/status headings, '- title', or HTML comments",
                    path.display(),
                    index + 1
                )
            };
            if in_comment {
                if let Some((_, tail)) = trimmed.split_once("-->") {
                    ensure!(tail.trim().is_empty(), "{}", invalid());
                    in_comment = false;
                }
                continue;
            }
            if let Some(comment) = trimmed.strip_prefix("<!--") {
                let comment = comment.trim_start();
                if comment.starts_with("kiroku-format") {
                    ensure!(
                        !has_format_header,
                        "{}:{}: duplicate kiroku-format header",
                        path.display(),
                        index + 1
                    );
                    ensure!(
                        days.is_empty(),
                        "{}:{}: kiroku-format header must precede the first date heading",
                        path.display(),
                        index + 1
                    );
                    parse_format_version(comment)
                        .with_context(|| format!("{}:{}", path.display(), index + 1))?;
                    has_format_header = true;
                }
                if let Some((_, tail)) = trimmed.split_once("-->") {
                    ensure!(tail.trim().is_empty(), "{}", invalid());
                } else {
                    in_comment = true;
                }
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            if let Some(date_text) = line.strip_prefix("# ") {
                let date = parse_date(date_text)
                    .with_context(|| format!("{}:{}", path.display(), index + 1))?;
                ensure!(
                    path.file_stem().and_then(|s| s.to_str())
                        == Some(date.format("%Y-%m").to_string().as_str()),
                    "{}:{}: date does not belong to this monthly file",
                    path.display(),
                    index + 1
                );
                if let Some(previous) = days.last_mut() {
                    ensure!(
                        previous.date < date,
                        "{}:{}: days must be unique and chronological",
                        path.display(),
                        index + 1
                    );
                    previous.end = index;
                }
                days.push(Day {
                    date,
                    items: vec![],
                    headings: [usize::MAX; 3],
                    start: index,
                    end: lines.len(),
                });
                status = None;
            } else if let Some(label) = line.strip_prefix("## ") {
                let next = Status::ALL
                    .into_iter()
                    .find(|s| s.label() == label)
                    .with_context(invalid)?;
                let day = days
                    .last_mut()
                    .context("A status heading needs a preceding date heading")?;
                ensure!(
                    day.headings[next.index()] == usize::MAX,
                    "{}:{}: duplicate status section",
                    path.display(),
                    index + 1
                );
                if let Some(previous) = status {
                    ensure!(
                        previous < next.index(),
                        "{}:{}: sections must be To do, Doing, Done in that order",
                        path.display(),
                        index + 1
                    );
                }
                day.headings[next.index()] = index;
                status = Some(next.index());
            } else if let Some(title) = line.strip_prefix("- ") {
                let current = status.context("An item needs a preceding status heading")?;
                let title = validate_title(title)
                    .with_context(|| format!("{}:{}", path.display(), index + 1))?;
                days.last_mut()
                    .context("An item needs a date heading")?
                    .items
                    .push(Item {
                        title: title.to_owned(),
                        status: Status::ALL[current],
                        line: index,
                    });
            } else {
                bail!("{}", invalid());
            }
        }
        ensure!(!in_comment, "{}: unclosed HTML comment", path.display());
        for day in &days {
            ensure!(
                !day.headings.contains(&usize::MAX),
                "{}: {} needs To do, Doing, and Done sections",
                path.display(),
                day.date
            );
        }
        Ok(Self {
            path,
            original,
            lines,
            days,
            newline,
            has_format_header,
        })
    }

    fn insert_item(
        &mut self,
        day_index: usize,
        status: Status,
        title: &str,
        position: Option<usize>,
    ) -> Result<()> {
        let day = &self.days[day_index];
        let section: Vec<_> = day.items.iter().filter(|i| i.status == status).collect();
        let position = position.unwrap_or(section.len() + 1);
        ensure!(
            (1..=section.len() + 1).contains(&position),
            "Position must be between 1 and {} in {}",
            section.len() + 1,
            status.label()
        );
        let insertion = section.get(position - 1).map_or_else(
            || {
                section
                    .last()
                    .map_or(day.headings[status.index()] + 1, |item| item.line + 1)
            },
            |item| item.line,
        );
        if insertion > 0 && !self.lines[insertion - 1].ends_with('\n') {
            self.lines[insertion - 1].push_str(self.newline);
        }
        self.lines
            .insert(insertion, format!("- {title}{}", self.newline));
        Ok(())
    }

    fn ensure_day(&mut self, date: NaiveDate) -> Result<usize> {
        let index = self.days.partition_point(|day| day.date < date);
        if self.days.get(index).is_some_and(|day| day.date == date) {
            return Ok(index);
        }
        let insertion = self
            .days
            .get(index)
            .map_or(self.lines.len(), |day| day.start);
        let nl = self.newline;
        let mut lines = vec![];
        if insertion > 0 {
            if !self.lines[insertion - 1].ends_with('\n') {
                self.lines[insertion - 1].push_str(nl);
            }
            lines.push(nl.to_owned());
        }
        lines.push(format!("# {date}{nl}"));
        lines.push(nl.to_owned());
        for status in Status::ALL {
            lines.push(format!("## {}{nl}", status.label()));
            lines.push(nl.to_owned());
        }
        self.lines.splice(insertion..insertion, lines);
        self.reparse()?;
        Ok(index)
    }

    fn reparse(&mut self) -> Result<()> {
        self.days = Self::parse(self.path.clone(), Some(self.lines.concat()))?.days;
        Ok(())
    }

    fn remove_items(&mut self, items: &[Item]) -> Result<()> {
        let lines = items.iter().map(|item| item.line).collect::<BTreeSet<_>>();
        for line in lines.into_iter().rev() {
            self.lines.remove(line);
        }
        self.reparse()
    }

    fn insert_items(
        &mut self,
        day_index: usize,
        status: Status,
        items: &[Item],
        position: Option<usize>,
    ) -> Result<()> {
        for (offset, item) in items.iter().enumerate() {
            let position = position
                .map(|position| {
                    position
                        .checked_add(offset)
                        .context("Position is too large")
                })
                .transpose()?;
            self.insert_item(day_index, status, &item.title, position)?;
            self.reparse()?;
        }
        Ok(())
    }

    fn change_items(&mut self, day_index: usize, items: &[Item], change: ItemChange) -> Result<()> {
        match change {
            ItemChange::Remove => self.remove_items(items)?,
            ItemChange::Edit(title) => {
                let title = validate_title(&title)?;
                for item in items {
                    let raw = &self.lines[item.line];
                    let ending = &raw[raw.trim_end_matches(['\r', '\n']).len()..];
                    self.lines[item.line] = format!("- {title}{ending}");
                }
                self.reparse()?;
            }
            ItemChange::Move(status, position) => {
                let moved = items
                    .iter()
                    .filter(|item| position.is_some() || item.status != status)
                    .cloned()
                    .collect::<Vec<_>>();
                self.remove_items(&moved)?;
                self.insert_items(day_index, status, &moved, position)?;
            }
            ItemChange::Reorder(direction, steps) => {
                ensure!(steps > 0, "Reorder steps must be greater than zero");
                let selected = items.iter().map(|item| item.line).collect::<BTreeSet<_>>();
                for status in Status::ALL {
                    let section = self.days[day_index]
                        .items
                        .iter()
                        .filter(|item| item.status == status)
                        .collect::<Vec<_>>();
                    let indices = section
                        .iter()
                        .enumerate()
                        .filter_map(|(index, item)| selected.contains(&item.line).then_some(index))
                        .collect::<Vec<_>>();
                    let mut order = vec![None; section.len()];
                    for (rank, &index) in indices.iter().enumerate() {
                        let target = match direction {
                            Direction::Up => index.saturating_sub(steps).max(rank),
                            Direction::Down => index
                                .saturating_add(steps)
                                .min(section.len() - indices.len() + rank),
                        };
                        order[target] = Some(index);
                    }
                    let mut unselected = section.iter().enumerate().filter_map(|(index, item)| {
                        (!selected.contains(&item.line)).then_some(index)
                    });
                    let contents = section
                        .iter()
                        .map(|item| {
                            self.lines[item.line]
                                .trim_end_matches(['\r', '\n'])
                                .to_owned()
                        })
                        .collect::<Vec<_>>();
                    // Keep comments and line endings at their original positions.
                    for (item, index) in section.iter().zip(order) {
                        let index = index.unwrap_or_else(|| unselected.next().unwrap());
                        let raw = &self.lines[item.line];
                        let ending = &raw[raw.trim_end_matches(['\r', '\n']).len()..];
                        self.lines[item.line] = format!("{}{ending}", contents[index]);
                    }
                }
                self.reparse()?;
            }
        }
        Ok(())
    }

    fn contents(&self) -> String {
        let mut contents = self.lines.concat();
        // Legacy files are format 1 after validation. Annotate only an actual
        // change, so reads and no-op edits never rewrite or back up a file.
        if !self.has_format_header && self.original.as_deref() != Some(contents.as_str()) {
            contents = format!(
                "<!-- kiroku-format: {STORAGE_FORMAT_VERSION} -->{}{}{contents}",
                self.newline, self.newline
            );
        }
        contents
    }

    fn save(&self, close_previous_months: bool) -> Result<()> {
        let contents = self.contents();
        if close_previous_months {
            crate::persistence::save(&self.path, self.original.as_deref(), &contents)
        } else {
            crate::persistence::save_retaining_months(
                &self.path,
                self.original.as_deref(),
                &contents,
            )
        }
    }

    fn save_move_to(&self, destination: &Self, close_previous_months: bool) -> Result<()> {
        crate::persistence::save_move(
            crate::persistence::LogUpdate {
                path: &destination.path,
                original: destination.original.as_deref(),
                content: &destination.contents(),
            },
            crate::persistence::LogUpdate {
                path: &self.path,
                original: self.original.as_deref(),
                content: &self.contents(),
            },
            close_previous_months,
        )
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    Add(String, Status),
    AddAt(String, Status, usize),
    Move(usize, Status),
    MoveAt(usize, Status, usize),
    Reorder(usize, Direction, usize),
    Edit(usize, String),
    Remove(usize),
}

#[derive(Clone, Debug)]
pub enum ItemChange {
    Move(Status, Option<usize>),
    Reorder(Direction, usize),
    Edit(String),
    Remove,
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn neighbor(self, index: usize) -> Option<usize> {
        match self {
            Self::Up => index.checked_sub(1),
            Self::Down => index.checked_add(1),
        }
    }
}

#[derive(Clone)]
pub struct Store {
    pub dir: PathBuf,
}

struct SelectedDocuments {
    documents: BTreeMap<PathBuf, Document>,
    items: Vec<(PathBuf, Item)>,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn paths(&self) -> Result<Vec<PathBuf>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e).context("Cannot read log directory"),
        };
        let mut paths = vec![];
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.is_ascii()
                && name.len() == 10
                && name.ends_with(".md")
                && parse_date(&format!("{}-01", &name[..7])).is_ok()
            {
                ensure!(
                    entry.file_type()?.is_file(),
                    "{} is not a regular log file",
                    entry.path().display()
                );
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn active_document_on(&self, today: NaiveDate) -> Result<Option<(Document, usize)>> {
        let month = today.format("%Y-%m").to_string();
        for path in self.paths()?.into_iter().rev() {
            if path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem > month.as_str())
            {
                continue;
            }
            let document = Document::read(path)?;
            if let Some(index) = document.days.iter().rposition(|day| day.date <= today) {
                return Ok(Some((document, index)));
            }
        }
        Ok(None)
    }

    pub fn active(&self) -> Result<Option<Day>> {
        self.active_on(Local::now().date_naive())
    }

    /// Future records do not become active until their calendar date arrives.
    pub fn active_on(&self, today: NaiveDate) -> Result<Option<Day>> {
        Ok(self
            .active_document_on(today)?
            .map(|(document, index)| document.days[index].clone()))
    }

    pub fn days(&self, range: Option<(NaiveDate, NaiveDate)>) -> Result<Vec<Day>> {
        let mut days = vec![];
        for path in self.paths()? {
            if let Some((from, to)) = range {
                let month = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .context("Invalid month filename")?;
                if month < from.format("%Y-%m").to_string().as_str()
                    || month > to.format("%Y-%m").to_string().as_str()
                {
                    continue;
                }
            }
            days.extend(
                Document::read(path)?
                    .days
                    .into_iter()
                    .filter(|d| range.is_none_or(|(from, to)| d.date >= from && d.date <= to)),
            );
        }
        Ok(days)
    }

    pub fn previous(&self, before: NaiveDate) -> Result<Option<Day>> {
        for path in self.paths()?.into_iter().rev() {
            let month = path
                .file_stem()
                .and_then(|s| s.to_str())
                .context("Invalid month filename")?;
            if month > before.format("%Y-%m").to_string().as_str() {
                continue;
            }
            if let Some(day) = Document::read(path)?
                .days
                .into_iter()
                .rev()
                .find(|d| d.date < before)
            {
                return Ok(Some(day));
            }
        }
        Ok(None)
    }

    fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.dir).context("Cannot create data directory")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.dir.join(".kiroku.lock"))?;
        lock.lock_exclusive()?;
        Ok(lock)
    }

    pub fn new_day(&self, today: NaiveDate) -> Result<bool> {
        let _lock = self.lock()?;
        let previous = self.active_document_on(today)?;
        if previous
            .as_ref()
            .is_some_and(|(document, index)| document.days[*index].date == today)
        {
            return Ok(false);
        }
        let unfinished: Vec<_> = previous
            .as_ref()
            .map(|(document, index)| {
                document.days[*index]
                    .numbered()
                    .into_iter()
                    .filter(|item| item.status != Status::Done)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let path = self.dir.join(format!("{}.md", today.format("%Y-%m")));
        let (mut document, source) = if let Some((mut previous, _)) = previous {
            // Item lines are ordered by section and file position. Remove in
            // reverse so comments, Done entries, and all other days stay in place.
            for item in unfinished.iter().rev() {
                previous.lines.remove(item.line);
            }
            if !unfinished.is_empty() {
                previous.reparse()?;
            }
            if previous.path == path {
                (previous, None)
            } else {
                let source = (!unfinished.is_empty()).then_some(previous);
                (Document::read_or_empty(path)?, source)
            }
        } else {
            (Document::read_or_empty(path)?, None)
        };
        let index = document.ensure_day(today)?;
        let lines = unfinished
            .iter()
            .map(|item| format!("- {}{}", item.title, document.newline));
        let insertion = document.days[index].headings[Status::Todo.index()] + 1;
        document.lines.splice(insertion..insertion, lines);
        document.reparse()?;
        if let Some(source) = source {
            source.save_move_to(&document, true)?;
        } else {
            document.save(true)?;
        }
        Ok(true)
    }

    pub fn change(&self, change: Change, expected: Option<&Day>) -> Result<Day> {
        match change {
            Change::Move(number, status) => {
                return self.change_items(&[number], ItemChange::Move(status, None), expected);
            }
            Change::MoveAt(number, status, position) => {
                return self.change_items(
                    &[number],
                    ItemChange::Move(status, Some(position)),
                    expected,
                );
            }
            Change::Reorder(number, direction, steps) => {
                return self.change_items(
                    &[number],
                    ItemChange::Reorder(direction, steps),
                    expected,
                );
            }
            Change::Edit(number, title) => {
                return self.change_items(&[number], ItemChange::Edit(title), expected);
            }
            Change::Remove(number) => {
                return self.change_items(&[number], ItemChange::Remove, expected);
            }
            _ => (),
        }
        let (title, status, position) = match change {
            Change::Add(title, status) => (title, status, None),
            Change::AddAt(title, status, position) => (title, status, Some(position)),
            _ => unreachable!(),
        };
        let title = validate_title(&title)?;
        let _lock = self.lock()?;
        let today = Local::now().date_naive();
        let Some((mut document, index)) = self.active_document_on(today)? else {
            ensure!(
                expected.is_none(),
                "The active day changed outside this view. Reload it before editing"
            );
            return self.insert_on_date(today, title, status, position);
        };
        if let Some(expected) = expected {
            ensure!(
                document.days[index].matches(expected),
                "The active day changed outside this view. Reload it before editing"
            );
        }
        document.insert_item(index, status, title, position)?;
        document.reparse()?;
        document.save(true)?;
        Ok(document.days[index].clone())
    }

    /// Apply a selection against one original active-day snapshot and save once.
    pub fn change_items(
        &self,
        numbers: &[usize],
        change: ItemChange,
        expected: Option<&Day>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let (mut document, index) = self
            .active_document_on(Local::now().date_naive())?
            .context("No day has been started. Run `krk new-day` first")?;
        if let Some(expected) = expected {
            ensure!(
                document.days[index].matches(expected),
                "The active day changed outside this view. Reload it before editing"
            );
        }
        let items = selected_items(&document.days[index], numbers)?;
        document.change_items(index, &items, change)?;
        document.save(true)?;
        Ok(document.days[index].clone())
    }

    /// Remove one item from a recorded date without changing the active day.
    /// Numbers are one-based within that date; a snapshot guards the selection.
    pub fn remove_on_date(
        &self,
        date: NaiveDate,
        number: usize,
        expected_source: Option<&Day>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let path = self.dir.join(format!("{}.md", date.format("%Y-%m")));
        let mut document = Document::read_or_empty(path)?;
        let index = document
            .days
            .iter()
            .position(|day| day.date == date)
            .with_context(|| format!("No recorded day for {date}"))?;
        let day = &document.days[index];
        if let Some(expected) = expected_source {
            ensure!(
                day.matches(expected),
                "The source day changed outside this view. Rerun the search or listing before removing"
            );
        }
        let line = day
            .numbered()
            .get(number.wrapping_sub(1))
            .copied()
            .with_context(|| format!("No source item {number} on {date}"))?
            .line;
        document.lines.remove(line);
        document.reparse()?;
        // Removing from history or a future plan does not close other months.
        document.save(false)?;
        Ok(document.days[index].clone())
    }

    // The caller holds the directory lock; validate every source before editing.
    fn read_selection(&self, selected: &[(&Day, usize)]) -> Result<SelectedDocuments> {
        ensure!(!selected.is_empty(), "Select at least one item");
        let mut documents = BTreeMap::new();
        let mut items = Vec::new();
        let mut seen = BTreeSet::new();
        for &(expected, number) in selected {
            if !seen.insert((expected.date, number)) {
                continue;
            }
            let path = self
                .dir
                .join(format!("{}.md", expected.date.format("%Y-%m")));
            let document = match documents.entry(path.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(Document::read_or_empty(path.clone())?)
                }
            };
            let day = document
                .days
                .iter()
                .find(|day| day.date == expected.date)
                .with_context(|| format!("No recorded day for {}", expected.date))?;
            ensure!(
                day.matches(expected),
                "The source day changed outside this view. Rerun the search or listing before changing items"
            );
            let item = selected_items(day, &[number])?.pop().unwrap();
            items.push((path, item));
        }
        Ok(SelectedDocuments { documents, items })
    }

    /// Remove listed results from their original dates, saving each month once.
    pub fn remove_results(&self, selected: &[(&Day, usize)]) -> Result<()> {
        let _lock = self.lock()?;
        let mut selection = self.read_selection(selected)?;
        let mut grouped = BTreeMap::<_, Vec<_>>::new();
        for (path, item) in selection.items {
            grouped.entry(path).or_default().push(item);
        }
        for (path, items) in grouped {
            selection
                .documents
                .get_mut(&path)
                .unwrap()
                .remove_items(&items)?;
        }
        let contents = selection
            .documents
            .values()
            .map(Document::contents)
            .collect::<Vec<_>>();
        let updates = selection
            .documents
            .values()
            .zip(&contents)
            .map(|(document, content)| crate::persistence::LogUpdate {
                path: &document.path,
                original: document.original.as_deref(),
                content,
            })
            .collect::<Vec<_>>();
        crate::persistence::save_batch(&updates)
    }

    /// Copy listed results in display order into one destination, saving once.
    pub fn repeat_results(
        &self,
        date: Option<NaiveDate>,
        selected: &[(&Day, usize)],
        status: Status,
        expected_active: Option<&Day>,
        position: Option<usize>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let selection = self.read_selection(selected)?;
        let (mut document, index) = if let Some(date) = date {
            let path = self.dir.join(format!("{}.md", date.format("%Y-%m")));
            let mut document = Document::read_or_empty(path)?;
            let index = document.ensure_day(date)?;
            (document, index)
        } else {
            let (document, index) = self
                .active_document_on(Local::now().date_naive())?
                .context("No day has been started. Run `krk new-day` first")?;
            if let Some(expected) = expected_active {
                ensure!(
                    document.days[index].matches(expected),
                    "The active day changed outside this view. Reload it before repeating"
                );
            }
            (document, index)
        };
        let items = selection
            .items
            .into_iter()
            .map(|(_, item)| item)
            .collect::<Vec<_>>();
        document.insert_items(index, status, &items, position)?;
        document.save(date.is_none())?;
        Ok(document.days[index].clone())
    }

    /// Add to an explicit date, creating an empty day without rollover.
    pub fn add_on_date(&self, date: NaiveDate, title: &str, status: Status) -> Result<Day> {
        self.add_on_date_at(date, title, status, None)
    }

    pub fn add_on_date_at(
        &self,
        date: NaiveDate,
        title: &str,
        status: Status,
        position: Option<usize>,
    ) -> Result<Day> {
        let title = validate_title(title)?;
        let _lock = self.lock()?;
        self.insert_on_date(date, title, status, position)
    }

    // The caller holds the directory lock through loading, insertion and save.
    fn insert_on_date(
        &self,
        date: NaiveDate,
        title: &str,
        status: Status,
        position: Option<usize>,
    ) -> Result<Day> {
        let path = self.dir.join(format!("{}.md", date.format("%Y-%m")));
        let mut document = Document::read_or_empty(path)?;
        let index = document.ensure_day(date)?;
        document.insert_item(index, status, title, position)?;
        document.reparse()?;
        // Backfilling or planning a month does not close the active month.
        document.save(false)?;
        Ok(document.days[index].clone())
    }

    /// Move an item into an explicit date and section, removing its source entry.
    /// Without `from`, numbers refer to the active day. A snapshot protects TUI
    /// selections; destination contents are loaded while holding the directory lock.
    pub fn move_on_date_at(
        &self,
        date: NaiveDate,
        from: Option<NaiveDate>,
        number: usize,
        status: Status,
        expected_source: Option<&Day>,
        position: Option<usize>,
    ) -> Result<Day> {
        self.move_items_on_date_at(date, from, &[number], status, expected_source, position)
    }

    pub fn move_items_on_date_at(
        &self,
        date: NaiveDate,
        from: Option<NaiveDate>,
        numbers: &[usize],
        status: Status,
        expected_source: Option<&Day>,
        position: Option<usize>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let (mut source, source_index) = if let Some(from) = from {
            let path = self.dir.join(format!("{}.md", from.format("%Y-%m")));
            let document = Document::read_or_empty(path)?;
            let index = document
                .days
                .iter()
                .position(|day| day.date == from)
                .with_context(|| format!("No recorded day for {from}"))?;
            (document, index)
        } else {
            self.active_document_on(Local::now().date_naive())?
                .context("No day has been started. Run `krk new-day` first")?
        };
        let source_day = &source.days[source_index];
        if let Some(expected) = expected_source {
            ensure!(
                source_day.matches(expected),
                "The source day changed outside this view. Reload it before moving"
            );
        }
        let items = selected_items(source_day, numbers)?;
        let destination_path = self.dir.join(format!("{}.md", date.format("%Y-%m")));
        if date == source_day.date {
            source.change_items(source_index, &items, ItemChange::Move(status, position))?;
            source.save(false)?;
            return Ok(source.days[source_index].clone());
        }
        source.remove_items(&items)?;
        if source.path == destination_path {
            let index = source.ensure_day(date)?;
            source.insert_items(index, status, &items, position)?;
            source.save(false)?;
            return Ok(source.days[index].clone());
        }
        let mut destination = Document::read_or_empty(destination_path)?;
        let index = destination.ensure_day(date)?;
        destination.insert_items(index, status, &items, position)?;
        // Prepare both logs and backups before replacing either. Save and flush
        // the destination first so an interrupted cross-month move cannot lose
        // the item. A partial move may leave a duplicate, with recovery guidance.
        source.save_move_to(&destination, false)?;
        Ok(destination.days[index].clone())
    }

    /// Copy from a recorded source into an explicit destination date. Without
    /// `from`, numbers still refer to the active day, not the destination.
    pub fn repeat_on_date(
        &self,
        date: NaiveDate,
        from: Option<NaiveDate>,
        number: usize,
        status: Status,
        expected_source: Option<&Day>,
    ) -> Result<Day> {
        self.repeat_on_date_at(date, from, number, status, expected_source, None)
    }

    pub fn repeat_on_date_at(
        &self,
        date: NaiveDate,
        from: Option<NaiveDate>,
        number: usize,
        status: Status,
        expected_source: Option<&Day>,
        position: Option<usize>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let source = if let Some(from) = from {
            self.days(Some((from, from)))?
                .pop()
                .with_context(|| format!("No recorded day for {from}"))?
        } else {
            self.active()?
                .context("No day has been started. Run `krk new-day` first")?
        };
        if let Some(expected) = expected_source {
            ensure!(
                source.matches(expected),
                "The source day changed outside this view. Reload it before repeating"
            );
        }
        let item = source
            .numbered()
            .get(number.wrapping_sub(1))
            .copied()
            .with_context(|| format!("No source item {number} on {}", source.date))?;
        self.insert_on_date(date, &item.title, status, position)
    }

    /// Copy a source item into the active day without changing its original entry.
    /// Numbers are one-based within the source day. `from` selects another
    /// recorded day; None selects the active day. TUI snapshots guard both the selection
    /// and the destination against changes since the view was loaded.
    pub fn repeat(
        &self,
        from: Option<NaiveDate>,
        number: usize,
        status: Status,
        expected_source: Option<&Day>,
        expected_active: Option<&Day>,
    ) -> Result<Day> {
        self.repeat_at(from, number, status, expected_source, expected_active, None)
    }

    pub fn repeat_at(
        &self,
        from: Option<NaiveDate>,
        number: usize,
        status: Status,
        expected_source: Option<&Day>,
        expected_active: Option<&Day>,
        position: Option<usize>,
    ) -> Result<Day> {
        let _lock = self.lock()?;
        let (mut document, day_index) = self
            .active_document_on(Local::now().date_naive())?
            .context("No day has been started. Run `krk new-day` first")?;
        let active = &document.days[day_index];
        if let Some(expected) = expected_active {
            ensure!(
                active.matches(expected),
                "The active day changed outside this view. Reload it before repeating"
            );
        }
        let source = if let Some(date) = from {
            ensure!(
                date != active.date,
                "--from must select a different recorded day; use an active-day number otherwise"
            );
            self.days(Some((date, date)))?
                .pop()
                .with_context(|| format!("No recorded day for {date}"))?
        } else {
            active.clone()
        };
        if let Some(expected) = expected_source {
            ensure!(
                source.matches(expected),
                "The source day changed outside this view. Reload it before repeating"
            );
        }
        let item = source
            .numbered()
            .get(number.wrapping_sub(1))
            .copied()
            .with_context(|| format!("No source item {number} on {}", source.date))?
            .clone();
        document.insert_item(day_index, status, &item.title, position)?;
        document.reparse()?;
        document.save(true)?;
        Ok(document.days[day_index].clone())
    }
}
