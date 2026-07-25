use crate::storage;
use color_eyre::{Result, eyre::bail};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

// enum Mode {
//     Normal,
//     Editing,
// }

pub struct App {
    items: Vec<Item>,
    state: ListState,
}

impl App {
    pub fn new() -> Result<App> {
        let items = get_items()?;
        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            items: items,
            state: state,
        })
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => self.up(),
                    KeyCode::Down | KeyCode::Char('j') => self.down(),
                    _ => {}
                }
            }
        }
    }

    fn up(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn down(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i == self.items.len() - 1 {
                    i
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn render(&mut self, frame: &mut Frame) {
        let screen_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(frame.area());

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let (line, style) = if item.category == ItemCategory::Date {
                    let style = Style::default().fg(Color::Magenta);
                    let line = Line::from(Span::raw(item.text.as_str()));
                    (line, style)
                } else if item.category == ItemCategory::Todo {
                    let style = Style::default().fg(Color::LightYellow);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(item.text.as_str())]);
                    (line, style)
                } else if item.category == ItemCategory::Doing {
                    let style = Style::default().fg(Color::LightBlue);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(item.text.as_str())]);
                    (line, style)
                } else if item.category == ItemCategory::Completed {
                    let style = Style::default().fg(Color::LightGreen);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(item.text.as_str())]);
                    (line, style)
                } else {
                    let style = Style::default().fg(Color::Red);
                    let line = Line::from(Span::raw(item.text.as_str()));
                    (line, style)
                };

                ListItem::new(line).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" TODO List "))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, screen_chunks[0], &mut self.state);
    }
}

fn get_items() -> Result<Vec<Item>> {
    let mut items = Vec::new();

    let lines = storage::get_lines()?;
    for line in lines {
        if let Ok(text) = line {
            if text.is_empty() {
                continue;
            }

            let date_line = text.starts_with(storage::DATE_PREFIX);
            if date_line {
                let date_str = text.trim_start_matches(storage::DATE_PREFIX);
                let item = Item {
                    category: ItemCategory::Date,
                    text: date_str.to_string(),
                };
                items.push(item);
            } else {
                let category: ItemCategory;
                if text.starts_with(storage::TODO_PREFIX) {
                    category = ItemCategory::Todo;
                } else if text.starts_with(storage::DOING_PREFIX) {
                    category = ItemCategory::Doing;
                } else if text.starts_with(storage::COMPLETED_PREFIX) {
                    category = ItemCategory::Completed;
                } else {
                    bail!("unrecognized line prefix")
                }

                let item = Item {
                    category: category,
                    text: text,
                };
                items.push(item);
            }
        }
    }

    Ok(items)
}

struct Item {
    category: ItemCategory,
    text: String,
}

#[derive(PartialEq)]
enum ItemCategory {
    Date,
    Todo,
    Doing,
    Completed,
}
