use anyhow::{Context, Result, ensure};
use chrono::{Local, NaiveDate};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use kiroku::{
    listing,
    storage::{Change, Day, Direction, Status, Store, parse_date},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal},
};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

enum Prompt {
    Add,
    Search,
    Edit(usize),
    Remove(usize),
    Move {
        source: Day,
        number: usize,
        status: Status,
    },
}

struct Search {
    filter: listing::Filter,
    results: Vec<(usize, usize)>, // Source day index and original item index.
    selected: usize,
}

struct TitleSuggestion {
    title: String,
    last_used: NaiveDate,
    last_completed: Option<NaiveDate>,
}

fn title_history(days: &[Day]) -> Vec<TitleSuggestion> {
    let mut titles = BTreeMap::new();
    for day in days.iter().rev() {
        for item in &day.items {
            let entry = titles
                .entry(item.title.clone())
                .or_insert_with(|| TitleSuggestion {
                    title: item.title.clone(),
                    last_used: day.date,
                    last_completed: None,
                });
            if item.status == Status::Done && entry.last_completed.is_none() {
                entry.last_completed = Some(day.date);
            }
        }
    }
    let mut titles: Vec<_> = titles.into_values().collect();
    titles.sort_by(|a, b| {
        b.last_used
            .cmp(&a.last_used)
            .then_with(|| a.title.cmp(&b.title))
    });
    titles
}

struct App {
    store: Store,
    days: Vec<Day>,
    current: usize,
    selected: usize,
    message: String,
    prompt: Option<Prompt>,
    input: String,
    title_history: Vec<TitleSuggestion>,
    suggestion: Option<usize>,
    add_position: Option<usize>,
    search: Option<Search>,
    help: bool,
    error_popup: Option<String>,
    list_state: ListState,
}

impl App {
    fn day(&self) -> Option<&Day> {
        self.days.get(self.current)
    }
    fn active(&self) -> Option<NaiveDate> {
        self.active_index().map(|index| self.days[index].date)
    }
    fn active_index(&self) -> Option<usize> {
        let today = Local::now().date_naive();
        self.days.iter().rposition(|day| day.date <= today)
    }
    fn editable(&self) -> bool {
        self.day().is_some_and(|d| Some(d.date) == self.active())
    }

    fn reload(&mut self) -> Result<()> {
        let date = self.day().map(|d| d.date);
        let search_selection = self.search.as_ref().and_then(|search| {
            search
                .results
                .get(search.selected)
                .map(|&(day, item)| (self.days[day].date, item))
        });
        self.days = self.store.days(None)?;
        self.title_history = title_history(&self.days);
        self.suggestion = None;
        self.current = date
            .and_then(|date| self.days.iter().position(|d| d.date == date))
            .unwrap_or_else(|| self.active_index().unwrap_or(0));
        let length = self.day().map_or(0, |d| d.items.len());
        self.selected = self.selected.min(length.saturating_sub(1));
        self.rebuild_search(search_selection);
        Ok(())
    }

    fn rebuild_search(&mut self, selected: Option<(NaiveDate, usize)>) {
        if let Some(search) = &mut self.search {
            let filter = &search.filter;
            search.results = self
                .days
                .iter()
                .enumerate()
                .rev()
                .flat_map(|(day_index, day)| {
                    day.numbered()
                        .into_iter()
                        .enumerate()
                        .filter(move |(_, item)| filter.matches(item))
                        .map(move |(item_index, _)| (day_index, item_index))
                })
                .collect();
            search.selected = selected
                .and_then(|(date, item)| {
                    search.results.iter().position(|&(day_index, item_index)| {
                        self.days[day_index].date == date && item_index == item
                    })
                })
                .unwrap_or(0);
            self.list_state = ListState::default();
        }
    }

    fn search_prompt(&mut self) {
        self.input = self
            .search
            .as_ref()
            .map_or_else(String::new, |search| search.filter.query.clone());
        self.suggestion = None;
        self.prompt = Some(Prompt::Search);
    }

    fn start_today(&mut self) -> Result<()> {
        let today = Local::now().date_naive();
        let rollover_selection = self.search.as_ref().and_then(|search| {
            let &(day_index, item_index) = search.results.get(search.selected)?;
            let day = &self.days[day_index];
            let items = day.numbered();
            (Some(day.date) == self.active()).then(|| {
                let done = items[item_index].status == Status::Done;
                let index = items[..item_index]
                    .iter()
                    .filter(|item| (item.status == Status::Done) == done)
                    .count();
                (if done { day.date } else { today }, index)
            })
        });
        let created = self.store.new_day(today)?;
        self.reload()?;
        if created && rollover_selection.is_some() {
            self.rebuild_search(rollover_selection);
        }
        self.current = self.active_index().unwrap_or(0);
        self.selected = 0;
        self.message = if created {
            "Started today with unfinished items and recurring todos."
        } else {
            "Today is already started; no changes made."
        }
        .into();
        Ok(())
    }

    fn search_key(&mut self, key: KeyEvent) -> Result<bool> {
        let search = self.search.as_mut().unwrap();
        let length = search.results.len();
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Esc => {
                self.search = None;
                self.list_state = ListState::default();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                search.selected = (search.selected + 1).min(length.saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => search.selected = search.selected.saturating_sub(1),
            KeyCode::Home | KeyCode::Char('g') => search.selected = 0,
            KeyCode::End | KeyCode::Char('G') => search.selected = length.saturating_sub(1),
            KeyCode::Char('/') => self.search_prompt(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('f') => {
                search.filter.status = match search.filter.status {
                    None => Some(Status::Done),
                    Some(Status::Done) => Some(Status::Todo),
                    Some(Status::Todo) => Some(Status::Doing),
                    Some(Status::Doing) => None,
                };
                self.rebuild_search(None);
            }
            KeyCode::Char('c') if length > 0 => {
                let (day, item) = search.results[search.selected];
                self.repeat_item(day, item + 1)?;
            }
            KeyCode::Char('m') if length > 0 => {
                let (day, item) = search.results[search.selected];
                self.move_prompt(day, item + 1);
            }
            KeyCode::Enter if length > 0 => {
                let (day, item) = search.results[search.selected];
                self.current = day;
                self.selected = item;
                self.search = None;
                self.list_state = ListState::default();
            }
            KeyCode::Char('r') => {
                self.reload()?;
                self.message = "Reloaded files.".into();
            }
            KeyCode::Char('n') => self.start_today()?,
            _ => (),
        }
        Ok(false)
    }

    fn apply(&mut self, change: Change) -> Result<()> {
        let first_add =
            matches!(change, Change::Add(..) | Change::AddAt(..)) && self.active().is_none();
        ensure!(
            self.editable() || first_add,
            "This edit requires the active day; use m to move an item from another date"
        );
        let expected = if first_add { None } else { self.day().cloned() };
        self.store.change(change, expected.as_ref())?;
        self.reload()?;
        if first_add {
            self.current = self.active_index().unwrap_or(0);
            self.selected = 0;
            self.list_state = ListState::default();
        }
        self.message = "Saved. Item numbers may have changed.".into();
        Ok(())
    }

    fn suggestions(&self) -> Vec<&TitleSuggestion> {
        let query = self.input.trim().to_lowercase();
        self.title_history
            .iter()
            .filter(|entry| entry.title.to_lowercase().contains(&query))
            .take(5)
            .collect()
    }

    fn reorder(&mut self, direction: Direction) -> Result<()> {
        let items = self.day().unwrap().numbered();
        let target = direction.neighbor(self.selected).filter(|&index| {
            items
                .get(index)
                .is_some_and(|item| item.status == items[self.selected].status)
        });
        self.apply(Change::Reorder(self.selected + 1, direction, 1))?;
        if let Some(target) = target {
            self.selected = target;
            self.message = "Saved item order.".into();
        } else {
            self.message = "Already at the edge of this section.".into();
        }
        Ok(())
    }

    fn repeat_item(&mut self, day_index: usize, number: usize) -> Result<()> {
        let source = self.days[day_index].clone();
        let active_index = self
            .active_index()
            .context("No day has been started. Run `krk new-day` first")?;
        let active = self.days[active_index].clone();
        let from = (source.date != active.date).then_some(source.date);
        self.store
            .repeat(from, number, Status::Todo, Some(&source), Some(&active))?;
        self.search = None;
        self.reload()?;
        self.current = self.active_index().unwrap();
        self.selected = self
            .day()
            .unwrap()
            .numbered()
            .into_iter()
            .take_while(|item| item.status == Status::Todo)
            .count()
            .saturating_sub(1);
        self.list_state = ListState::default();
        self.message = "Copied to the active day's To do. Press s to start working.".into();
        Ok(())
    }

    fn move_prompt(&mut self, day_index: usize, number: usize) {
        let source = self.days[day_index].clone();
        let status = source.numbered()[number - 1].status;
        self.input = Local::now().date_naive().to_string();
        self.prompt = Some(Prompt::Move {
            source,
            number,
            status,
        });
    }

    fn move_item(&mut self, source: &Day, number: usize, status: Status) -> Result<()> {
        let date = parse_date(&self.input)?;
        let unchanged = date == source.date && source.numbered()[number - 1].status == status;
        let destination = self.store.move_on_date_at(
            date,
            Some(source.date),
            number,
            status,
            Some(source),
            None,
        )?;
        let selected = if unchanged {
            number - 1
        } else {
            destination
                .numbered()
                .iter()
                .rposition(|item| item.status == status)
                .unwrap()
        };
        self.search = None;
        self.reload()?;
        self.current = self.days.iter().position(|day| day.date == date).unwrap();
        self.selected = selected;
        self.list_state = ListState::default();
        self.message = format!(
            "Moved to {date}, {}. Item numbers may have changed.",
            status.label()
        );
        Ok(())
    }

    fn key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        if self.error_popup.is_some() {
            match key.code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Enter | KeyCode::Esc => self.error_popup = None,
                _ => (),
            }
            return Ok(false);
        }
        if let Some(prompt) = &self.prompt {
            if let Prompt::Remove(number) = prompt {
                let number = *number;
                match key.code {
                    KeyCode::Char('y' | 'Y') => {
                        self.prompt = None;
                        self.apply(Change::Remove(number))?;
                    }
                    KeyCode::Char('n' | 'N') | KeyCode::Esc => self.prompt = None,
                    _ => (),
                }
            } else {
                match key.code {
                    KeyCode::Esc => {
                        self.prompt = None;
                        self.input.clear();
                        self.suggestion = None;
                    }
                    KeyCode::Left | KeyCode::Right
                        if matches!(prompt, Prompt::Add)
                            && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        let end = self.active_index().map_or(1, |index| {
                            self.days[index]
                                .items
                                .iter()
                                .filter(|item| item.status == Status::Todo)
                                .count()
                                + 1
                        });
                        let position = self.add_position.unwrap_or(end);
                        self.add_position = Some(if key.code == KeyCode::Left {
                            position.saturating_sub(1).max(1)
                        } else {
                            position.saturating_add(1).min(end)
                        });
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                        self.suggestion = None;
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.clear();
                        self.suggestion = None;
                    }
                    KeyCode::Tab | KeyCode::BackTab if matches!(prompt, Prompt::Move { .. }) => {
                        if let Some(Prompt::Move { status, .. }) = &mut self.prompt {
                            let index = Status::ALL
                                .iter()
                                .position(|entry| entry == status)
                                .unwrap();
                            let step = if key.code == KeyCode::Tab { 1 } else { 2 };
                            *status = Status::ALL[(index + step) % Status::ALL.len()];
                        }
                    }
                    KeyCode::Down | KeyCode::Tab if matches!(prompt, Prompt::Add) => {
                        let length = self.suggestions().len();
                        if length > 0 {
                            self.suggestion = Some(self.suggestion.map_or(0, |n| (n + 1) % length));
                        }
                    }
                    KeyCode::Up | KeyCode::BackTab if matches!(prompt, Prompt::Add) => {
                        let length = self.suggestions().len();
                        self.suggestion = match self.suggestion {
                            Some(0) => None,
                            Some(n) => Some(n - 1),
                            None => length.checked_sub(1),
                        };
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        self.input.push(c);
                        self.suggestion = None;
                    }
                    KeyCode::Enter => {
                        if let Prompt::Move {
                            source,
                            number,
                            status,
                        } = prompt
                        {
                            let source = source.clone();
                            let number = *number;
                            let status = *status;
                            self.move_item(&source, number, status)?;
                            self.prompt = None;
                            self.input.clear();
                            return Ok(false);
                        }
                        if matches!(prompt, Prompt::Search) {
                            let status =
                                self.search.as_ref().and_then(|search| search.filter.status);
                            self.search = Some(Search {
                                filter: listing::Filter::new(&self.input, status)?,
                                results: vec![],
                                selected: 0,
                            });
                            self.rebuild_search(None);
                            self.prompt = None;
                            self.input.clear();
                            self.message =
                                "Search covers all recorded days. Press c to copy a match.".into();
                            return Ok(false);
                        }
                        let change = match prompt {
                            Prompt::Add => {
                                let title = self
                                    .suggestion
                                    .and_then(|n| {
                                        self.suggestions().get(n).map(|entry| entry.title.clone())
                                    })
                                    .unwrap_or_else(|| self.input.clone());
                                match self.add_position {
                                    Some(position) => Change::AddAt(title, Status::Todo, position),
                                    None => Change::Add(title, Status::Todo),
                                }
                            }
                            Prompt::Edit(n) => Change::Edit(*n, self.input.clone()),
                            _ => unreachable!(),
                        };
                        self.apply(change)?;
                        self.prompt = None;
                        self.input.clear();
                    }
                    _ => (),
                }
            }
            return Ok(false);
        }
        if self.help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help = false;
            } else if key.code == KeyCode::Char('q') {
                return Ok(true);
            }
            return Ok(false);
        }
        if self.search.is_some() {
            return self.search_key(key);
        }
        let number = self.selected + 1;
        let length = self.day().map_or(0, |d| d.items.len());
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') => self.search_prompt(),
            KeyCode::Char('J') if length > 0 => self.reorder(Direction::Down)?,
            KeyCode::Char('K') if length > 0 => self.reorder(Direction::Up)?,
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(length.saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Home | KeyCode::Char('g') => self.selected = 0,
            KeyCode::End | KeyCode::Char('G') => self.selected = length.saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => {
                self.current = self.current.saturating_sub(1);
                self.selected = 0;
                self.list_state = ListState::default();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.current = (self.current + 1).min(self.days.len().saturating_sub(1));
                self.selected = 0;
                self.list_state = ListState::default();
            }
            KeyCode::Char('r') => {
                self.reload()?;
                self.message = "Reloaded files.".into();
            }
            KeyCode::Char('n') => self.start_today()?,
            KeyCode::Char('a') => {
                ensure!(
                    self.editable() || self.active().is_none(),
                    "Navigate to the active day to add items"
                );
                self.input.clear();
                self.suggestion = None;
                self.add_position = None;
                self.prompt = Some(Prompt::Add);
            }
            KeyCode::Char('e') if length > 0 => {
                ensure!(self.editable(), "Renaming requires the active day");
                self.input = self.day().unwrap().numbered()[self.selected].title.clone();
                self.prompt = Some(Prompt::Edit(number));
            }
            KeyCode::Char('x') if length > 0 => {
                ensure!(self.editable(), "Removing requires the active day");
                self.prompt = Some(Prompt::Remove(number));
            }
            KeyCode::Char('t') if length > 0 => self.apply(Change::Move(number, Status::Todo))?,
            KeyCode::Char('s') if length > 0 => self.apply(Change::Move(number, Status::Doing))?,
            KeyCode::Char('d') if length > 0 => self.apply(Change::Move(number, Status::Done))?,
            KeyCode::Char('c') if length > 0 => self.repeat_item(self.current, number)?,
            KeyCode::Char('m') if length > 0 => self.move_prompt(self.current, number),
            _ => (),
        }
        Ok(false)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
            Constraint::Length(3),
        ])
        .split(area);
        let today = Local::now().date_naive();
        let heading = if let Some(search) = &self.search {
            format!(
                "Kiroku | {} {} | {} | active {}{}",
                search.results.len(),
                if search.results.len() == 1 {
                    "match"
                } else {
                    "matches"
                },
                search.filter.status.map_or("all statuses", Status::label),
                self.active()
                    .map_or_else(|| "none".into(), |date| date.to_string()),
                if self.active().is_some_and(|date| date != today) {
                    " (not today)"
                } else {
                    ""
                },
            )
        } else {
            match self.day() {
                Some(day) => format!(
                    "Kiroku  |  {}  |  {}{}",
                    day.date,
                    if self.editable() {
                        "active"
                    } else if day.date > today {
                        "future (m to move)"
                    } else {
                        "history (m to move)"
                    },
                    if day.date != today {
                        "  |  not today"
                    } else {
                        "  |  today"
                    }
                ),
                None => "Kiroku  |  No recorded days — press a to add your first item".into(),
            }
        };
        frame.render_widget(
            Paragraph::new(heading)
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan)),
            layout[0],
        );
        let mut rows = vec![];
        let mut selected_row = None;
        if let Some(search) = &self.search {
            let numbered: Vec<_> = self.days.iter().map(Day::numbered).collect();
            let label_width = search
                .results
                .iter()
                .map(|&(day_index, item_index)| {
                    if Some(self.days[day_index].date) == self.active() {
                        (item_index + 1).to_string().len()
                    } else {
                        listing::letters(item_index).len()
                    }
                })
                .max()
                .unwrap_or(1);
            for (index, &(day_index, item_index)) in search.results.iter().enumerate() {
                let day = &self.days[day_index];
                let item = numbered[day_index][item_index];
                let label = if Some(day.date) == self.active() {
                    (item_index + 1).to_string()
                } else {
                    listing::letters(item_index)
                };
                rows.push(ListItem::new(format!(
                    "{}  {} {:<7} {}",
                    day.date,
                    listing::item_label(&label, label_width),
                    format!("[{}]", item.status.label()),
                    item.title
                )));
                if index == search.selected {
                    selected_row = Some(index);
                }
            }
            if rows.is_empty() {
                rows.push(ListItem::new(
                    "No matching items. Press / to change the search or f to change status.",
                ));
            }
        } else if let Some(day) = self.day() {
            let label_width = if self.editable() {
                day.items.len().to_string().len()
            } else {
                listing::letters(day.items.len().saturating_sub(1)).len()
            };
            let mut index = 0;
            for status in Status::ALL {
                let items: Vec<_> = day
                    .numbered()
                    .into_iter()
                    .filter(|i| i.status == status)
                    .collect();
                if items.is_empty() {
                    continue;
                }
                rows.push(ListItem::new(Line::from(Span::styled(
                    status.label(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))));
                for item in items {
                    if index == self.selected {
                        selected_row = Some(rows.len());
                    }
                    let label = if self.editable() {
                        (index + 1).to_string()
                    } else {
                        listing::letters(index)
                    };
                    rows.push(ListItem::new(format!(
                        "  {} {}",
                        listing::item_label(&label, label_width),
                        item.title
                    )));
                    index += 1;
                }
            }
        } else {
            rows.push(ListItem::new("Adding your first item starts today."));
        }
        self.list_state.select(selected_row);
        let title = self.search.as_ref().map_or_else(
            || " Work log ".into(),
            |search| format!(" Search: {} ", search.filter.query),
        );
        let list = List::new(rows)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");
        frame.render_stateful_widget(list, layout[1], &mut self.list_state);
        frame.render_widget(
            Paragraph::new(self.message.as_str())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::Yellow)),
            layout[2],
        );
        let shortcuts = if self.search.is_some() {
            "↑↓/jk matches  c copy  m move  Enter view  / search  f status\nn new day  r reload  Esc back  ? help  q quit"
        } else {
            "↑↓/jk items  K/J reorder  ←→/hl days  a add  c copy  m move  / search\ns doing  d done  t todo  e edit  x remove  n new day  r reload  ? help  q quit"
        };
        frame.render_widget(
            Paragraph::new(shortcuts).wrap(Wrap { trim: false }),
            layout[3],
        );
        if self.help {
            let text = "↑↓ or j/k: navigate items\nK/J (Shift-k/j): reorder up/down within a section\n←→ or h/l: browse recorded days\ng/G or Home/End: first/last item\na: add a title to the active day's To do\nc: copy selected item from any day to active To do\nm: move selected item to a date and section\n/: search titles across all recorded days\nSearch: f cycles all/Done/To do/Doing; Enter views source\nSearch: c copies; m moves; Esc returns to browsing\ns: Doing   d: Done   t: To do\ne: rename   x: remove (confirm with y)\nn: explicitly start today\nr: reload after editing the Markdown files\n\nHistorical items can be copied or explicitly moved with m.\nOther edits require the active day.\nPrompt: type, Backspace, Ctrl-U to clear, Enter to save.\nMove prompt: enter YYYY-MM-DD; Tab/Shift-Tab selects a section.\nAdd prompt: ↑↓/Tab selects a previous title; Ctrl-←/→ chooses insertion position.\nEsc cancels a prompt. Ctrl-C quits.\n\n? or Esc closes help; q quits.";
            popup(frame, area, " Keyboard help ", text, 26);
        }
        if let Some(prompt) = &self.prompt {
            let (title, text) = match prompt {
                Prompt::Add => {
                    let mut text = format!(
                        "{}\n\nPosition: {}. Ctrl-←/→ changes it.\n↑↓/Tab selects a previous title; Enter saves.\n",
                        self.input,
                        self.add_position
                            .map_or_else(|| "end (append)".into(), |n| n.to_string())
                    );
                    for (n, entry) in self.suggestions().iter().enumerate() {
                        let marker = if self.suggestion == Some(n) {
                            "›"
                        } else {
                            " "
                        };
                        let date = entry.last_completed.map_or_else(
                            || format!("used {}", entry.last_used),
                            |date| format!("done {date}"),
                        );
                        text.push_str(&format!("{marker} {} ({date})\n", entry.title));
                    }
                    (" Add todo — Enter saves, Esc cancels ", text)
                }
                Prompt::Edit(_) => (
                    " Edit title — Enter saves, Esc cancels ",
                    self.input.clone(),
                ),
                Prompt::Search => (
                    " Search titles — Enter lists, Esc cancels ",
                    format!(
                        "{}\n\nMatches text anywhere, ignoring case, across all days.\nLeave empty to list all items; f filters by status.",
                        self.input
                    ),
                ),
                Prompt::Remove(n) => (
                    " Remove item ",
                    format!("Remove active-day item {n}? Press y to remove, n/Esc to cancel."),
                ),
                Prompt::Move {
                    source,
                    number,
                    status,
                } => (
                    " Move item — Enter saves, Esc cancels ",
                    format!(
                        "{}\nFrom: {}\nDate: {}\nSection: {} (Tab/Shift-Tab changes it)\nType YYYY-MM-DD; Ctrl-U clears the date. Removes the source entry.",
                        source.numbered()[number - 1].title,
                        source.date,
                        self.input,
                        status.label()
                    ),
                ),
            };
            popup(
                frame,
                area,
                title,
                &text,
                match prompt {
                    Prompt::Add => 12,
                    Prompt::Move { .. } => 9,
                    _ => 6,
                },
            );
        }
        if let Some(error) = &self.error_popup {
            popup(
                frame,
                area,
                " Error — Enter/Esc closes; q quits ",
                error,
                16,
            );
        }
    }
}

fn popup(frame: &mut ratatui::Frame, area: Rect, title: &str, text: &str, height: u16) {
    let width = area.width.saturating_sub(4).min(76);
    let height = height.min(area.height);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(title)),
        rect,
    );
}

pub fn run(store: Store) -> Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "The TUI needs an interactive terminal; use `krk list` for plain output"
    );
    let mut app = App {
        store,
        days: vec![],
        current: 0,
        selected: 0,
        message: String::new(),
        prompt: None,
        input: String::new(),
        title_history: vec![],
        suggestion: None,
        add_position: None,
        search: None,
        help: false,
        error_popup: None,
        list_state: ListState::default(),
    };
    app.reload()?;
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        drop(Restore);
        old_hook(info);
    }));
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        // Block on input: no idle animation or polling loop.
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match app.key(key) {
                    Ok(true) => break,
                    Ok(false) => (),
                    Err(error) => {
                        app.message = format!("{error:#}");
                        app.prompt = None;
                        app.input.clear();
                        // A stale selection must not be reused after an external change.
                        if let Err(error) = app.reload() {
                            app.message
                                .push_str(&format!("; could not reload: {error:#}"));
                        }
                        app.error_popup = Some(app.message.clone());
                    }
                }
            }
            Event::Resize(_, _) => (),
            _ => (),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn app(store: Store) -> App {
        let mut app = App {
            store,
            days: vec![],
            current: 0,
            selected: 0,
            message: String::new(),
            prompt: None,
            input: String::new(),
            title_history: vec![],
            suggestion: None,
            add_position: None,
            search: None,
            help: false,
            error_popup: None,
            list_state: ListState::default(),
        };
        app.reload().unwrap();
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        assert!(!app.key(KeyEvent::new(code, KeyModifiers::NONE)).unwrap());
    }

    #[test]
    fn new_day_key_adds_configured_todos_once_and_preserves_completed_history() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(config_dir.join("kiroku")).unwrap();
        let today = Local::now().date_naive();
        std::fs::write(
            config_dir.join("kiroku/config.toml"),
            format!(
                r#"
[[recurring_todos]]
title = "recurring"
days = ["{}"]
"#,
                today.format("%A").to_string().to_lowercase()
            ),
        )
        .unwrap();
        let store = Store::new(temp.path().join("logs"))
            .with_recurring_todos(kiroku::config::recurring_todos_with(&config_dir).unwrap())
            .unwrap();
        store
            .add_on_date(today - chrono::Duration::days(1), "recurring", Status::Done)
            .unwrap();
        store
            .add_on_date(today - chrono::Duration::days(1), "carry", Status::Doing)
            .unwrap();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.day().unwrap().date, today);
        assert_eq!(
            app.day()
                .unwrap()
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["carry", "recurring"]
        );
        assert!(
            app.day()
                .unwrap()
                .items
                .iter()
                .all(|item| item.status == Status::Todo)
        );
        assert_eq!(
            store.previous(today).unwrap().unwrap().items[0].status,
            Status::Done
        );
        app.selected = 1;
        press(&mut app, KeyCode::Char('d'));
        let path = store.dir.join(format!("{}.md", today.format("%Y-%m")));
        let original = std::fs::read(&path).unwrap();
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(std::fs::read(path).unwrap(), original);
        assert!(app.message.contains("already started"));
    }

    #[test]
    fn move_prompt_moves_a_historical_duplicate_to_the_chosen_section_and_selects_it() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let historical = today - chrono::Duration::days(35);
        store
            .add_on_date(historical, "duplicate 日本語", Status::Todo)
            .unwrap();
        store
            .add_on_date(historical, "duplicate 日本語", Status::Doing)
            .unwrap();
        store
            .add_on_date(today, "existing done", Status::Done)
            .unwrap();
        let mut app = app(store.clone());
        app.current = 0;
        app.selected = 1;
        press(&mut app, KeyCode::Char('m'));
        assert_eq!(app.input, today.to_string());
        press(&mut app, KeyCode::Tab); // Doing -> Done.
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Move item"));
        assert!(text.contains(&format!("From: {historical}")));
        assert!(text.contains("Section: Done"));
        press(&mut app, KeyCode::Enter);
        assert!(app.prompt.is_none());
        assert_eq!(app.day().unwrap().date, today);
        assert_eq!(app.selected, 1);
        assert_eq!(
            app.day().unwrap().numbered()[app.selected].title,
            "duplicate 日本語"
        );
        assert_eq!(
            app.day().unwrap().numbered()[app.selected].status,
            Status::Done
        );
        let source = store.days(Some((historical, historical))).unwrap();
        assert_eq!(source[0].items.len(), 1);
        assert_eq!(source[0].items[0].status, Status::Todo);
    }

    #[test]
    fn move_prompt_cancellation_invalid_dates_and_stale_sources_leave_files_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store.add_on_date(today, "source", Status::Todo).unwrap();
        let path = store.dir.join(format!("{}.md", today.format("%Y-%m")));
        let original = std::fs::read(&path).unwrap();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Char('m'));
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::BackTab);
        assert!(matches!(
            app.prompt,
            Some(Prompt::Move {
                status: Status::Todo,
                ..
            })
        ));
        press(&mut app, KeyCode::Esc);
        assert!(app.prompt.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        press(&mut app, KeyCode::Char('m'));
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .unwrap();
        for character in "2026-02-30".chars() {
            press(&mut app, KeyCode::Char(character));
        }
        assert!(
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('m'));
        let destination = today + chrono::Duration::days(35);
        app.input = destination.to_string();
        store
            .change(Change::Edit(1, "external edit".into()), None)
            .unwrap();
        let edited = std::fs::read(&path).unwrap();
        let error = app
            .key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap_err();
        assert!(error.to_string().contains("source day changed"));
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        assert!(
            !store
                .dir
                .join(format!("{}.md", destination.format("%Y-%m")))
                .exists()
        );
    }

    #[test]
    fn search_can_move_to_a_future_day_without_changing_the_active_day() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let historical = today - chrono::Duration::days(1);
        let future = today + chrono::Duration::days(1);
        store
            .add_on_date(historical, "historical match", Status::Done)
            .unwrap();
        store.add_on_date(today, "active", Status::Todo).unwrap();
        let mut app = app(store.clone());
        search(&mut app, "match");
        press(&mut app, KeyCode::Char('m'));
        app.input = future.to_string();
        press(&mut app, KeyCode::Tab); // Done -> To do.
        press(&mut app, KeyCode::Enter);
        assert!(app.search.is_none());
        assert_eq!(app.day().unwrap().date, future);
        assert_eq!(app.day().unwrap().items[0].status, Status::Todo);
        assert!(!app.editable());
        assert_eq!(app.active(), Some(today));
        assert!(store.previous(today).unwrap().unwrap().items.is_empty());
        assert_eq!(store.active().unwrap().unwrap().items[0].title, "active");
    }

    #[test]
    fn reorder_keys_follow_selection_and_keep_history_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store.new_day(today - chrono::Duration::days(1)).unwrap();
        store
            .change(Change::Add("historical".into(), Status::Done), None)
            .unwrap();
        store.new_day(today).unwrap();
        for status in Status::ALL {
            for title in ["first", "second", "third"] {
                store
                    .change(Change::Add(title.into(), status), None)
                    .unwrap();
            }
        }
        let mut app = app(store.clone());
        for start in [0, 3, 6] {
            app.selected = start;
            press(&mut app, KeyCode::Char('K'));
            assert_eq!(app.selected, start);
            press(&mut app, KeyCode::Char('J'));
            press(&mut app, KeyCode::Char('J'));
            assert_eq!(app.selected, start + 2);
            assert_eq!(app.day().unwrap().numbered()[app.selected].title, "first");
            press(&mut app, KeyCode::Char('J'));
            assert_eq!(app.selected, start + 2);
            press(&mut app, KeyCode::Char('K'));
            assert_eq!(app.selected, start + 1);
            assert_eq!(app.day().unwrap().numbered()[app.selected].title, "first");
        }
        let active = store.active().unwrap().unwrap();
        store
            .change(Change::Edit(1, "external edit".into()), None)
            .unwrap();
        assert!(
            app.key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT))
                .is_err()
        );
        app.reload().unwrap();
        press(&mut app, KeyCode::Left);
        assert!(
            app.key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT))
                .is_err()
        );
        assert_eq!(
            store.previous(today).unwrap().unwrap().numbered()[0].title,
            "historical"
        );
        assert_eq!(active.numbered()[0].title, "second");
    }

    #[test]
    fn add_position_keys_insert_and_reset_without_changing_suggestion_navigation() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        store
            .change(Change::Add("first".into(), Status::Todo), None)
            .unwrap();
        store
            .change(Change::Add("last".into(), Status::Todo), None)
            .unwrap();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Char('a'));
        for _ in 0..4 {
            app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
                .unwrap();
        }
        assert_eq!(app.add_position, Some(1));
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))
            .unwrap();
        assert_eq!(app.add_position, Some(2));
        app.input = "middle".into();
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            store
                .active()
                .unwrap()
                .unwrap()
                .numbered()
                .iter()
                .map(|i| i.title.as_str())
                .collect::<Vec<_>>(),
            ["first", "middle", "last"]
        );
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.add_position, None);
        for _ in 0..5 {
            app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))
                .unwrap();
        }
        assert_eq!(app.add_position, Some(4));
        press(&mut app, KeyCode::Down);
        assert!(app.suggestion.is_some());
        press(&mut app, KeyCode::Esc);
        assert_eq!(store.active().unwrap().unwrap().items.len(), 3);
    }

    fn search(app: &mut App, query: &str) {
        press(app, KeyCode::Char('/'));
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .unwrap();
        for c in query.chars() {
            press(app, KeyCode::Char(c));
        }
        press(app, KeyCode::Enter);
    }

    #[test]
    fn first_add_prompt_creates_today_only_when_saved() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().join("logs"));
        let mut app = app(store.clone());
        assert!(!store.dir.exists());
        press(&mut app, KeyCode::Char('a'));
        assert!(matches!(app.prompt, Some(Prompt::Add)));
        press(&mut app, KeyCode::Esc);
        assert!(!store.dir.exists());
        press(&mut app, KeyCode::Char('a'));
        assert!(
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_err()
        );
        assert!(store.active().unwrap().is_none());
        for c in "first todo".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert!(app.prompt.is_none());
        assert!(app.editable());
        assert_eq!(app.day().unwrap().date, Local::now().date_naive());
        let day = store.active().unwrap().unwrap();
        assert_eq!(day.items.len(), 1);
        assert_eq!(day.items[0].title, "first todo");
        assert_eq!(day.items[0].status, Status::Todo);
    }

    #[test]
    fn future_records_do_not_redirect_tui_edits_copies_or_start_today() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let future = today + chrono::Duration::days(1);
        store.add_on_date(today, "current", Status::Todo).unwrap();
        store
            .add_on_date(future, "future plan", Status::Todo)
            .unwrap();
        let mut app = app(store.clone());
        assert_eq!(app.day().unwrap().date, today);
        assert!(app.editable());
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            store.active().unwrap().unwrap().items[0].status,
            Status::Done
        );
        press(&mut app, KeyCode::Right);
        assert_eq!(app.day().unwrap().date, future);
        assert!(!app.editable());
        assert!(app.apply(Change::Remove(1)).is_err());
        press(&mut app, KeyCode::Char('c'));
        assert_eq!(app.day().unwrap().date, today);
        assert!(app.editable());
        assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
        assert_eq!(
            store.days(Some((future, future))).unwrap()[0].items.len(),
            1
        );
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.day().unwrap().date, today);
        assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
    }

    #[test]
    fn future_only_tui_addition_starts_today_and_selects_it() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store
            .add_on_date(
                today + chrono::Duration::days(1),
                "future plan",
                Status::Todo,
            )
            .unwrap();
        let mut app = app(store.clone());
        assert!(app.active().is_none());
        assert!(!app.editable());
        assert!(
            app.repeat_item(0, 1)
                .unwrap_err()
                .to_string()
                .contains("new-day")
        );
        press(&mut app, KeyCode::Char('a'));
        for c in "today todo".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.day().unwrap().date, today);
        assert!(app.editable());
        assert_eq!(
            store.active().unwrap().unwrap().items[0].title,
            "today todo"
        );
        let days = store.days(None).unwrap();
        assert_eq!(days[1].items[0].title, "future plan");
    }

    #[test]
    fn search_filters_results_and_copies_the_original_item_to_today() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let source_date = today - chrono::Duration::days(2);
        store.new_day(source_date).unwrap();
        store
            .change(Change::Add("unrelated todo".into(), Status::Todo), None)
            .unwrap();
        store
            .change(Change::Add("meeting draft".into(), Status::Doing), None)
            .unwrap();
        store
            .change(Change::Add("möte 日本語".into(), Status::Done), None)
            .unwrap();
        store
            .change(Change::Add("same möte".into(), Status::Done), None)
            .unwrap();
        store.new_day(today - chrono::Duration::days(1)).unwrap();
        store
            .change(Change::Add("möte archived".into(), Status::Done), None)
            .unwrap();
        store.new_day(today).unwrap();
        store
            .change(Change::Add("möte current".into(), Status::Doing), None)
            .unwrap();
        store
            .change(Change::Add("möte completed".into(), Status::Done), None)
            .unwrap();
        let before = store
            .days(Some((source_date, source_date)))
            .unwrap()
            .remove(0);
        let mut app = app(store.clone());
        search(&mut app, "MÖTE");
        assert_eq!(app.search.as_ref().unwrap().results.len(), 5);
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(
            app.search.as_ref().unwrap().filter.status,
            Some(Status::Done)
        );
        assert_eq!(app.search.as_ref().unwrap().results.len(), 4);
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.search.as_ref().unwrap().results[2], (0, 0));
        press(&mut app, KeyCode::Char('c'));
        assert!(app.search.is_none());
        assert!(app.editable());
        let active = store.active().unwrap().unwrap();
        assert_eq!(active.date, today);
        assert_eq!(active.numbered()[app.selected].title, "möte 日本語");
        assert_eq!(active.numbered()[app.selected].status, Status::Todo);
        let after = store
            .days(Some((source_date, source_date)))
            .unwrap()
            .remove(0);
        assert_eq!(
            before
                .items
                .iter()
                .map(|i| (&i.title, i.status))
                .collect::<Vec<_>>(),
            after
                .items
                .iter()
                .map(|i| (&i.title, i.status))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn search_can_start_today_without_losing_the_selected_source() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let source_date = today - chrono::Duration::days(2);
        store.new_day(source_date).unwrap();
        store
            .change(Change::Add("matching other".into(), Status::Todo), None)
            .unwrap();
        store
            .change(Change::Add("matching title".into(), Status::Doing), None)
            .unwrap();
        store.new_day(today - chrono::Duration::days(1)).unwrap();
        let mut app = app(store.clone());
        search(&mut app, "matching");
        press(&mut app, KeyCode::End);
        assert_eq!(app.search.as_ref().unwrap().selected, 1);
        assert_eq!(app.search.as_ref().unwrap().results[1], (1, 1));
        press(&mut app, KeyCode::Char('n'));
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.selected, 1);
        assert_eq!(search.results[search.selected], (2, 1));
        press(&mut app, KeyCode::Char('c'));
        let active = store.active().unwrap().unwrap();
        assert_eq!(active.date, today);
        assert_eq!(active.items.len(), 3); // Two moved forward, one explicitly copied.
        assert_eq!(app.selected, 2);
        assert_eq!(active.numbered()[app.selected].title, "matching title");
        assert!(
            store.days(Some((source_date, source_date))).unwrap()[0]
                .items
                .is_empty()
        );
        assert!(store.previous(today).unwrap().unwrap().items.is_empty());
    }

    #[test]
    fn starting_today_keeps_a_selected_done_search_result_on_its_historical_day() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);
        store
            .add_on_date(yesterday, "matching unfinished", Status::Doing)
            .unwrap();
        store
            .add_on_date(yesterday, "matching done", Status::Done)
            .unwrap();
        let mut app = app(store.clone());
        search(&mut app, "matching");
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Char('n'));
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.selected, 1);
        let (day_index, item_index) = search.results[search.selected];
        assert_eq!(app.days[day_index].date, yesterday);
        assert_eq!(
            app.days[day_index].numbered()[item_index].title,
            "matching done"
        );
    }

    #[test]
    fn search_navigation_refinement_status_and_cancellation_are_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store.new_day(today - chrono::Duration::days(1)).unwrap();
        store
            .change(Change::Add("todo match".into(), Status::Todo), None)
            .unwrap();
        store
            .change(Change::Add("doing match".into(), Status::Doing), None)
            .unwrap();
        store
            .change(Change::Add("done match".into(), Status::Done), None)
            .unwrap();
        store.new_day(today).unwrap();
        let paths: Vec<_> = store
            .days(None)
            .unwrap()
            .iter()
            .map(|d| store.dir.join(format!("{}.md", d.date.format("%Y-%m"))))
            .collect();
        let original: Vec<_> = paths
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Esc);
        assert!(app.search.is_none());
        search(&mut app, "");
        assert_eq!(app.search.as_ref().unwrap().results.len(), 3);
        for status in [
            Some(Status::Done),
            Some(Status::Todo),
            Some(Status::Doing),
            None,
        ] {
            press(&mut app, KeyCode::Char('f'));
            let search = app.search.as_ref().unwrap();
            assert_eq!(search.filter.status, status);
            assert!(search.results.iter().all(|&(day, item)| {
                status.is_none_or(|status| app.days[day].numbered()[item].status == status)
            }));
        }
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.search.as_ref().unwrap().selected, 2);
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Up);
        assert_eq!(app.search.as_ref().unwrap().selected, 0);
        press(&mut app, KeyCode::Char('f')); // Done only.
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('z'));
        press(&mut app, KeyCode::Esc);
        assert!(app.search.as_ref().unwrap().filter.query.is_empty());
        search(&mut app, "done");
        assert_eq!(
            app.search.as_ref().unwrap().filter.status,
            Some(Status::Done)
        );
        press(&mut app, KeyCode::Enter);
        assert!(app.search.is_none());
        assert_eq!(app.current, 0);
        assert_eq!(app.selected, 0); // Original source index, not result position.
        search(&mut app, "absent");
        assert!(app.search.as_ref().unwrap().results.is_empty());
        press(&mut app, KeyCode::Char('c'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Char('r'));
        assert!(app.search.as_ref().unwrap().results.is_empty());
        press(&mut app, KeyCode::Esc);
        assert!(app.search.is_none());
        for (path, original) in paths.iter().zip(original) {
            assert_eq!(std::fs::read(path).unwrap(), original);
        }
    }

    #[test]
    fn search_results_render_original_letters_and_reject_stale_copies() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        let source_date = today - chrono::Duration::days(1);
        store.new_day(source_date).unwrap();
        for n in 0..27 {
            store
                .change(
                    Change::Add(
                        if n == 26 {
                            "target title".into()
                        } else {
                            format!("other {n}")
                        },
                        Status::Done,
                    ),
                    None,
                )
                .unwrap();
        }
        store.new_day(today).unwrap();
        let mut app = app(store.clone());
        search(&mut app, "target");
        for (width, height) in [(80, 24), (20, 8), (3, 2)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            if width == 80 {
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains("1 match |"));
                assert!(text.contains("[AA] [Done]  target title"));
                assert!(text.contains(&source_date.to_string()));
            }
            press(&mut app, KeyCode::Char('/'));
            terminal.draw(|frame| app.draw(frame)).unwrap();
            press(&mut app, KeyCode::Esc);
        }
        let path = store
            .dir
            .join(format!("{}.md", source_date.format("%Y-%m")));
        let changed = std::fs::read_to_string(&path)
            .unwrap()
            .replace("target title", "external edit");
        std::fs::write(&path, &changed).unwrap();
        let error = app
            .key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE))
            .unwrap_err();
        assert!(error.to_string().contains("source day changed"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
        assert!(store.active().unwrap().unwrap().items.is_empty());
        app.reload().unwrap();
        assert!(app.search.as_ref().unwrap().results.is_empty());
    }

    #[test]
    fn copy_active_item_preserves_original_and_selects_copy_for_starting() {
        for status in Status::ALL {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            store.new_day(Local::now().date_naive()).unwrap();
            store
                .change(Change::Add("existing todo".into(), Status::Todo), None)
                .unwrap();
            store
                .change(Change::Add("möte 日本語".into(), status), None)
                .unwrap();
            let mut app = app(store.clone());
            app.selected = 1;
            press(&mut app, KeyCode::Char('c'));
            assert!(app.editable());
            assert_eq!(app.selected, if status == Status::Todo { 2 } else { 1 });
            let day = store.active().unwrap().unwrap();
            assert_eq!(day.items.len(), 3);
            assert_eq!(day.numbered()[app.selected].title, "möte 日本語");
            assert_eq!(day.numbered()[app.selected].status, Status::Todo);
            press(&mut app, KeyCode::Char('s'));
            let day = store.active().unwrap().unwrap();
            assert!(
                day.items
                    .iter()
                    .any(|i| i.title == "möte 日本語" && i.status == status)
            );
            assert!(
                day.items
                    .iter()
                    .any(|i| i.title == "möte 日本語" && i.status == Status::Doing)
            );
        }
    }

    #[test]
    fn copy_historical_item_jumps_to_active_day_and_preserves_history() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let earlier = kiroku::storage::parse_date("2026-09-30").unwrap();
        store.new_day(earlier).unwrap();
        store
            .change(Change::Add("123".into(), Status::Done), None)
            .unwrap();
        store
            .new_day(kiroku::storage::parse_date("2026-10-02").unwrap())
            .unwrap();
        store
            .change(Change::Add("already here".into(), Status::Todo), None)
            .unwrap();
        let original = std::fs::read(store.dir.join("2026-09.md")).unwrap();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Left);
        assert!(!app.editable());
        press(&mut app, KeyCode::Char('c'));
        assert!(app.editable());
        assert_eq!(app.selected, 1);
        press(&mut app, KeyCode::Char('s'));
        let day = store.active().unwrap().unwrap();
        assert_eq!(day.items.len(), 2);
        assert!(
            day.items
                .iter()
                .any(|i| i.title == "123" && i.status == Status::Doing)
        );
        assert_eq!(
            std::fs::read(store.dir.join("2026-09.md")).unwrap(),
            original
        );
    }

    #[test]
    fn copy_rejects_stale_historical_source_or_active_destination() {
        for change_source in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            let source_date = kiroku::storage::parse_date("2026-09-30").unwrap();
            store.new_day(source_date).unwrap();
            store
                .change(Change::Add("selected title".into(), Status::Done), None)
                .unwrap();
            store
                .new_day(kiroku::storage::parse_date("2026-10-02").unwrap())
                .unwrap();
            let mut app = app(store.clone());
            press(&mut app, KeyCode::Left);
            if change_source {
                let path = store.dir.join("2026-09.md");
                let text = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("selected title", "external edit");
                std::fs::write(path, text).unwrap();
            } else {
                store
                    .change(Change::Add("external addition".into(), Status::Todo), None)
                    .unwrap();
            }
            let path = store.dir.join("2026-10.md");
            let before = std::fs::read(&path).unwrap();
            let error = app
                .key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE))
                .unwrap_err();
            assert!(error.to_string().contains("day changed"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn add_suggests_unique_titles_and_reuses_exact_text_only_when_selected() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let first = kiroku::storage::parse_date("2026-09-29").unwrap();
        let second = kiroku::storage::parse_date("2026-09-30").unwrap();
        store.new_day(first).unwrap();
        store
            .change(Change::Add("möte 日本語".into(), Status::Done), None)
            .unwrap();
        store.new_day(second).unwrap();
        for _ in 0..2 {
            store
                .change(Change::Add("möte 日本語".into(), Status::Done), None)
                .unwrap();
        }
        store
            .change(Change::Add("never completed".into(), Status::Todo), None)
            .unwrap();
        store
            .new_day(kiroku::storage::parse_date("2026-10-02").unwrap())
            .unwrap();
        let source = std::fs::read(store.dir.join("2026-09.md")).unwrap();
        let mut app = app(store.clone());
        let never_completed = app
            .title_history
            .iter()
            .find(|i| i.title == "never completed")
            .unwrap();
        assert!(never_completed.last_completed.is_none());
        press(&mut app, KeyCode::Char('a'));
        for c in "MÖTE".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.suggestions().len(), 1);
        assert_eq!(app.suggestions()[0].last_completed, Some(second));
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.suggestion, Some(0));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("› möte "));
        assert!(rendered.contains("(done 2026-09-30)"));
        for (width, height) in [(20, 8), (3, 2)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
        }
        press(&mut app, KeyCode::Enter);
        assert!(
            store
                .active()
                .unwrap()
                .unwrap()
                .items
                .iter()
                .any(|i| i.title == "möte 日本語")
        );
        assert_eq!(std::fs::read(store.dir.join("2026-09.md")).unwrap(), source);
        // Matching text remains a new title unless a suggestion was selected.
        press(&mut app, KeyCode::Char('a'));
        for c in "mö".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert!(app.suggestion.is_none());
        press(&mut app, KeyCode::Enter);
        assert!(
            store
                .active()
                .unwrap()
                .unwrap()
                .items
                .iter()
                .any(|i| i.title == "mö")
        );
        let path = store.dir.join("2026-10.md");
        let before = std::fs::read(&path).unwrap();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Down);
        assert!(app.suggestion.is_some());
        press(&mut app, KeyCode::Up);
        assert!(app.suggestion.is_none());
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char('z'));
        assert!(app.suggestion.is_none());
        assert!(app.suggestions().is_empty());
        press(&mut app, KeyCode::Down);
        assert!(app.suggestion.is_none());
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .unwrap();
        assert!(app.input.is_empty());
        press(&mut app, KeyCode::Esc);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Tab);
        assert!(app.suggestion.is_none());
        press(&mut app, KeyCode::Esc);
    }

    #[test]
    fn keyboard_workflow_saves_and_historical_days_cannot_be_changed() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store.new_day(today - chrono::Duration::days(3)).unwrap();
        store
            .change(Change::Add("previous work".into(), Status::Todo), None)
            .unwrap();
        store
            .change(
                Change::Add("previous finished work".into(), Status::Done),
                None,
            )
            .unwrap();
        let mut app = app(store.clone());
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(
            store.active().unwrap().unwrap().numbered()[0].status,
            Status::Doing
        );
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('a'));
        for c in "möte 日本語".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            store.active().unwrap().unwrap().numbered()[0].title,
            "möte 日本語"
        );
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Esc); // editing without saving leaves the title alone
        assert_eq!(
            store.active().unwrap().unwrap().numbered()[0].title,
            "möte 日本語"
        );
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(store.active().unwrap().unwrap().items.len(), 1);
        press(&mut app, KeyCode::Left);
        assert!(!app.editable());
        assert!(
            app.key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE))
                .is_err()
        );
        assert_eq!(
            store.previous(today).unwrap().unwrap().items[0].status,
            Status::Done
        );
        press(&mut app, KeyCode::Right);
        assert!(app.editable());
    }

    #[test]
    fn bracketed_labels_align_titles_in_browsing_and_search() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        store.new_day(Local::now().date_naive()).unwrap();
        for n in 1..=12 {
            store
                .change(
                    Change::Add(
                        format!("alignment item {n}"),
                        if n <= 6 { Status::Todo } else { Status::Done },
                    ),
                    None,
                )
                .unwrap();
        }
        let mut app = app(store);
        for searching in [false, true] {
            if searching {
                search(&mut app, "alignment");
            }
            let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let buffer = terminal.backend().buffer();
            let lines: Vec<String> = buffer
                .content
                .chunks(100)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect())
                .filter(|line: &String| line.contains("alignment item"))
                .collect();
            assert_eq!(lines.len(), 12);
            let column = lines[0][..lines[0].find("alignment item").unwrap()]
                .chars()
                .count();
            for line in &lines {
                assert_eq!(
                    line[..line.find("alignment item").unwrap()].chars().count(),
                    column,
                    "{line}"
                );
            }
            assert!(lines.iter().any(|line| line.contains("[9]")));
            assert!(lines.iter().any(|line| line.contains("[10]")));
        }
    }

    #[test]
    fn render_normal_tiny_and_empty_terminals_without_panics() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let mut app = app(store.clone());
        for (width, height) in [(80, 24), (20, 8), (3, 2)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.help = true;
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.help = false;
            app.prompt = Some(Prompt::Add);
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.prompt = None;
            app.error_popup = Some(
                "Update is already visible. Do not repeat the command; inspect the file first."
                    .into(),
            );
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.error_popup = None;
        }
        store.new_day(Local::now().date_naive()).unwrap();
        store
            .change(Change::Add("weekly meeting".into(), Status::Todo), None)
            .unwrap();
        app.reload().unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("weekly meeting"));
        assert!(rendered.contains("active"));
    }

    #[test]
    fn recovery_popup_is_visible_and_blocks_edits_until_dismissed() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        store.new_day(Local::now().date_naive()).unwrap();
        store
            .change(Change::Add("meeting".into(), Status::Todo), None)
            .unwrap();
        let mut app = app(store.clone());
        app.error_popup = Some(
            "Update is already visible. Do not blindly repeat the command; inspect the file first."
                .into(),
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Update is already visible"));
        assert!(text.contains("Do not blindly repeat"));
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            store.active().unwrap().unwrap().items[0].status,
            Status::Todo
        );
        press(&mut app, KeyCode::Enter);
        assert!(app.error_popup.is_none());
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(
            store.active().unwrap().unwrap().items[0].status,
            Status::Done
        );
    }
}
