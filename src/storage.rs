use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, NaiveDate};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

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
    end: usize,
}

impl Day {
    pub fn numbered(&self) -> Vec<&Item> {
        Status::ALL
            .into_iter()
            .flat_map(|status| self.items.iter().filter(move |i| i.status == status))
            .collect()
    }
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
}

impl Document {
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
            if trimmed.starts_with("<!--") {
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
        })
    }

    fn insert_item(&mut self, day_index: usize, status: Status, title: &str) {
        let day = &self.days[day_index];
        let last = day
            .items
            .iter()
            .filter(|i| i.status == status)
            .map(|i| i.line)
            .max();
        let insertion = last.map_or(day.headings[status.index()] + 1, |line| line + 1);
        if insertion > 0 && !self.lines[insertion - 1].ends_with('\n') {
            self.lines[insertion - 1].push_str(self.newline);
        }
        self.lines
            .insert(insertion, format!("- {title}{}", self.newline));
    }

    fn reparse(&mut self) -> Result<()> {
        self.days = Self::parse(self.path.clone(), Some(self.lines.concat()))?.days;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        match (&self.original, fs::read_to_string(&self.path)) {
            (Some(old), Ok(current)) if old == &current => (),
            (None, Err(e)) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => bail!(
                "{} changed while Kiroku was editing it; retry after reviewing the file",
                self.path.display()
            ),
        }
        let parent = self.path.parent().context("Missing log directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if self.original.is_some() {
            temporary
                .as_file()
                .set_permissions(fs::metadata(&self.path)?.permissions())?;
        }
        temporary.write_all(self.lines.concat().as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|e| e.error)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    Add(String, Status),
    Move(usize, Status),
    Edit(usize, String),
    Remove(usize),
}

#[derive(Clone)]
pub struct Store {
    pub dir: PathBuf,
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

    fn active_document(&self) -> Result<Option<Document>> {
        for path in self.paths()?.into_iter().rev() {
            let document = Document::read(path)?;
            if !document.days.is_empty() {
                return Ok(Some(document));
            }
        }
        Ok(None)
    }

    pub fn active(&self) -> Result<Option<Day>> {
        Ok(self.active_document()?.and_then(|d| d.days.last().cloned()))
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
        let previous = self.active()?;
        if let Some(day) = &previous {
            if day.date == today {
                return Ok(false);
            }
            ensure!(
                day.date < today,
                "The last recorded day ({}) is later than today ({today}); check your clock or log files",
                day.date
            );
        }
        let path = self.dir.join(format!("{}.md", today.format("%Y-%m")));
        let mut document = match Document::read(path.clone()) {
            Ok(document) => document,
            Err(e) if !path.exists() => {
                // A missing file is expected; other read errors must not be hidden.
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                {
                    Document::parse(path, None)?
                } else {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        };
        let nl = document.newline;
        if let Some(last) = document.lines.last_mut() {
            if !last.ends_with('\n') {
                last.push_str(nl);
            }
            document.lines.push(nl.to_owned());
        }
        document.lines.push(format!("# {today}{nl}{nl}"));
        for status in Status::ALL {
            document.lines.push(format!("## {}{nl}", status.label()));
            if status == Status::Todo
                && let Some(previous) = &previous
            {
                for item in previous
                    .numbered()
                    .into_iter()
                    .filter(|i| i.status != Status::Done)
                {
                    document.lines.push(format!("- {}{nl}", item.title));
                }
            }
            document.lines.push(nl.to_owned());
        }
        document.reparse()?;
        document.save()?;
        Ok(true)
    }

    pub fn change(&self, change: Change, expected: Option<&Day>) -> Result<Day> {
        let _lock = self.lock()?;
        let mut document = self
            .active_document()?
            .context("No day has been started. Run `krk new-day` first")?;
        let day_index = document.days.len() - 1;
        let day = &document.days[day_index];
        if let Some(expected) = expected {
            ensure!(
                day.date == expected.date
                    && day
                        .items
                        .iter()
                        .map(|i| (&i.title, i.status))
                        .eq(expected.items.iter().map(|i| (&i.title, i.status))),
                "The active day changed outside this view. Reload it before editing"
            );
        }
        match change {
            Change::Add(title, status) => {
                document.insert_item(day_index, status, validate_title(&title)?)
            }
            change => {
                let number = match &change {
                    Change::Move(n, _) | Change::Edit(n, _) | Change::Remove(n) => *n,
                    _ => unreachable!(),
                };
                let item = day
                    .numbered()
                    .get(number.wrapping_sub(1))
                    .copied()
                    .with_context(|| {
                        format!(
                            "No active-day item {number}; run `krk list` to see current numbers"
                        )
                    })?
                    .clone();
                match change {
                    Change::Edit(_, title) => {
                        document.lines[item.line] = format!(
                            "- {}{}",
                            validate_title(&title)?,
                            if document.lines[item.line].ends_with('\n') {
                                document.newline
                            } else {
                                ""
                            }
                        )
                    }
                    Change::Remove(_) => {
                        document.lines.remove(item.line);
                    }
                    Change::Move(_, status) if item.status == status => return Ok(day.clone()),
                    Change::Move(_, status) => {
                        document.lines.remove(item.line);
                        document.reparse()?;
                        document.insert_item(day_index, status, &item.title);
                    }
                    _ => unreachable!(),
                }
            }
        }
        document.reparse()?;
        document.save()?;
        Ok(document.days[day_index].clone())
    }
}
