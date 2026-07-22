use crate::storage;
use color_eyre::{Result, eyre::bail};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

// enum Mode {
//     Normal,
//     Editing,
// }

pub struct App {
    items: Vec<Item>,
}

impl App {
    pub fn new() -> Result<App> {
        let items = get_items()?;

        Ok(App { items: items })
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
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

        // Create the List widget, wrapping it in a border box
        let list =
            List::new(items).block(Block::default().borders(Borders::ALL).title(" TODO List "));

        // Render the list inside our designated area chunk
        frame.render_widget(list, screen_chunks[0]);
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
