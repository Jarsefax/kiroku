use anyhow::{Result, ensure};
use chrono::{Local, NaiveDate};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use kiroku::{
    listing,
    storage::{Change, Day, Status, Store},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::io::{self, IsTerminal};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

enum Prompt {
    Add,
    Edit(usize),
    Remove(usize),
}

struct App {
    store: Store,
    days: Vec<Day>,
    current: usize,
    selected: usize,
    message: String,
    prompt: Option<Prompt>,
    input: String,
    help: bool,
    list_state: ListState,
}

impl App {
    fn day(&self) -> Option<&Day> {
        self.days.get(self.current)
    }
    fn active(&self) -> Option<NaiveDate> {
        self.days.last().map(|d| d.date)
    }
    fn editable(&self) -> bool {
        self.day().is_some_and(|d| Some(d.date) == self.active())
    }

    fn reload(&mut self) -> Result<()> {
        let date = self.day().map(|d| d.date);
        self.days = self.store.days(None)?;
        self.current = date
            .and_then(|date| self.days.iter().position(|d| d.date == date))
            .unwrap_or_else(|| self.days.len().saturating_sub(1));
        let length = self.day().map_or(0, |d| d.items.len());
        self.selected = self.selected.min(length.saturating_sub(1));
        Ok(())
    }

    fn apply(&mut self, change: Change) -> Result<()> {
        ensure!(
            self.editable(),
            "Historical days are read-only in the app; edit their Markdown files directly"
        );
        let expected = self.day().cloned();
        self.store.change(change, expected.as_ref())?;
        self.reload()?;
        self.message = "Saved. Item numbers may have changed.".into();
        Ok(())
    }

    fn key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
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
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.clear()
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        self.input.push(c)
                    }
                    KeyCode::Enter => {
                        let change = match prompt {
                            Prompt::Add => Change::Add(self.input.clone()),
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
        let number = self.selected + 1;
        let length = self.day().map_or(0, |d| d.items.len());
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('?') => self.help = true,
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
            KeyCode::Char('n') => {
                let created = self.store.new_day(Local::now().date_naive())?;
                self.reload()?;
                self.current = self.days.len().saturating_sub(1);
                self.selected = 0;
                self.message = if created {
                    "Started today; unfinished items carried into To do."
                } else {
                    "Today is already started; no changes made."
                }
                .into();
            }
            KeyCode::Char('a') => {
                ensure!(
                    self.editable(),
                    "Start a day with n, or navigate to the active day to add items"
                );
                self.input.clear();
                self.prompt = Some(Prompt::Add);
            }
            KeyCode::Char('e') if length > 0 => {
                ensure!(self.editable(), "Historical days are read-only in the app");
                self.input = self.day().unwrap().numbered()[self.selected].title.clone();
                self.prompt = Some(Prompt::Edit(number));
            }
            KeyCode::Char('x') if length > 0 => {
                ensure!(self.editable(), "Historical days are read-only in the app");
                self.prompt = Some(Prompt::Remove(number));
            }
            KeyCode::Char('t') if length > 0 => self.apply(Change::Move(number, Status::Todo))?,
            KeyCode::Char('s') if length > 0 => self.apply(Change::Move(number, Status::Doing))?,
            KeyCode::Char('d') if length > 0 => self.apply(Change::Move(number, Status::Done))?,
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
        let heading = match self.day() {
            Some(day) => format!(
                "Kiroku  |  {}  |  {}{}",
                day.date,
                if self.editable() {
                    "active"
                } else {
                    "history (read-only)"
                },
                if day.date != today {
                    "  |  not today"
                } else {
                    "  |  today"
                }
            ),
            None => "Kiroku  |  No recorded days — press n to start today".into(),
        };
        frame.render_widget(
            Paragraph::new(heading)
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan)),
            layout[0],
        );
        let mut rows = vec![];
        let mut selected_row = None;
        if let Some(day) = self.day() {
            let mut index = 0;
            for status in Status::ALL {
                rows.push(ListItem::new(Line::from(Span::styled(
                    status.label(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))));
                let items: Vec<_> = day
                    .numbered()
                    .into_iter()
                    .filter(|i| i.status == status)
                    .collect();
                if items.is_empty() {
                    rows.push(ListItem::new("  (empty)"));
                }
                for item in items {
                    if index == self.selected {
                        selected_row = Some(rows.len());
                    }
                    let label = if self.editable() {
                        (index + 1).to_string()
                    } else {
                        listing::letters(index)
                    };
                    rows.push(ListItem::new(format!("  {label}. {}", item.title)));
                    index += 1;
                }
            }
        } else {
            rows.push(ListItem::new("No days are created automatically."));
        }
        self.list_state.select(selected_row);
        let list = List::new(rows)
            .block(Block::default().borders(Borders::ALL).title(" Work log "))
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
        frame.render_widget(Paragraph::new("↑↓/jk items  ←→/hl days  a add  s doing  d done  t todo\ne edit  x remove  n new day  r reload  ? help  q quit").wrap(Wrap { trim: false }), layout[3]);
        if self.help {
            let text = "↑↓ or j/k: navigate items\n←→ or h/l: browse recorded days\ng/G or Home/End: first/last item\na: add a title to the active day's To do\ns: Doing   d: Done   t: To do\ne: rename   x: remove (confirm with y)\nn: explicitly start today\nr: reload after editing the Markdown files\n\nHistorical days are read-only in Kiroku.\nEdit their text files directly if needed.\nPrompt: type, Backspace, Ctrl-U to clear, Enter to save.\nEsc cancels a prompt. Ctrl-C quits.\n\n? or Esc closes help; q quits.";
            popup(frame, area, " Keyboard help ", text, 19);
        }
        if let Some(prompt) = &self.prompt {
            let (title, text) = match prompt {
                Prompt::Add => (" Add todo — Enter saves, Esc cancels ", self.input.clone()),
                Prompt::Edit(_) => (
                    " Edit title — Enter saves, Esc cancels ",
                    self.input.clone(),
                ),
                Prompt::Remove(n) => (
                    " Remove item ",
                    format!("Remove active-day item {n}? Press y to remove, n/Esc to cancel."),
                ),
            };
            popup(frame, area, title, &text, 6);
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
        help: false,
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
                            app.message = format!("{error:#}");
                        }
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
            help: false,
            list_state: ListState::default(),
        };
        app.reload().unwrap();
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        assert!(!app.key(KeyEvent::new(code, KeyModifiers::NONE)).unwrap());
    }

    #[test]
    fn keyboard_workflow_saves_and_historical_days_cannot_be_changed() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let today = Local::now().date_naive();
        store.new_day(today - chrono::Duration::days(3)).unwrap();
        store
            .change(Change::Add("previous work".into()), None)
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
            Status::Todo
        );
        press(&mut app, KeyCode::Right);
        assert!(app.editable());
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
        }
        store.new_day(Local::now().date_naive()).unwrap();
        store
            .change(Change::Add("weekly meeting".into()), None)
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
}
