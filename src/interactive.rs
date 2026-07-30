use crate::storage;
use color_eyre::{Result, eyre::bail};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub struct App {
    items: Vec<Item>,
    state: ListState,
    edit: Option<Item>,
}

impl App {
    pub fn new() -> Result<App> {
        let items = get_ordered_items()?;
        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            items: items,
            state: state,
            edit: None,
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
                    KeyCode::Char('e') => self.edit(),
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

    fn edit(&mut self) {
        if self.items.is_empty() {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                let item = self.items[i].clone();
                self.edit = Some(item);
            }
            None => {
                return;
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let screen_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(area);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let text = &item.text_in_file[item.prefix_length..];
                let (line, style) = if item.category == ItemCategory::Date {
                    let style = Style::default().fg(Color::Magenta);
                    let line = Line::from(Span::raw(text));
                    (line, style)
                } else if item.category == ItemCategory::Todo {
                    let style = Style::default().fg(Color::LightYellow);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(text)]);
                    (line, style)
                } else if item.category == ItemCategory::Doing {
                    let style = Style::default().fg(Color::LightBlue);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(text)]);
                    (line, style)
                } else if item.category == ItemCategory::Completed {
                    let style = Style::default().fg(Color::LightGreen);
                    let line = Line::from(vec![Span::raw("  "), Span::raw(text)]);
                    (line, style)
                } else {
                    let style = Style::default().fg(Color::Red);
                    let line = Line::from(Span::raw(text));
                    (line, style)
                };

                ListItem::new(line).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Kiroku "))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, screen_chunks[0], &mut self.state);

        if let Some(edit) = &mut self.edit {
            let center = area.centered(Constraint::Percentage(60), Constraint::Percentage(20));
            frame.render_widget(Clear, center);

            let popup = Block::bordered().title(" Edit ");
            let text = &edit.text_in_file[edit.prefix_length..];
            let paragraph = Paragraph::new(text).block(popup);
            frame.render_widget(paragraph, center);
        }
    }
}

fn get_ordered_items() -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut todos = Vec::new();
    let mut doings = Vec::new();
    let mut completeds = Vec::new();

    let lines = storage::get_lines()?;
    for (index_in_file, line) in lines.enumerate() {
        if let Ok(text_in_file) = line {
            if text_in_file.is_empty() {
                continue;
            }

            let is_date_line = text_in_file.starts_with(storage::DATE_PREFIX);
            if is_date_line {
                let item = Item::new(
                    ItemCategory::Date,
                    storage::DATE_PREFIX.len(),
                    text_in_file.to_string(),
                    index_in_file,
                );
                items.append(&mut todos);
                items.append(&mut doings);
                items.append(&mut completeds);
                items.push(item);
            } else {
                let category: ItemCategory;
                let prefix_length: usize;
                if text_in_file.starts_with(storage::TODO_PREFIX) {
                    category = ItemCategory::Todo;
                    prefix_length = storage::TODO_PREFIX.len();
                    let item = Item::new(
                        category,
                        prefix_length,
                        text_in_file.to_string(),
                        index_in_file,
                    );
                    todos.push(item);
                } else if text_in_file.starts_with(storage::DOING_PREFIX) {
                    category = ItemCategory::Doing;
                    prefix_length = storage::DOING_PREFIX.len();
                    let item = Item::new(
                        category,
                        prefix_length,
                        text_in_file.to_string(),
                        index_in_file,
                    );
                    doings.push(item);
                } else if text_in_file.starts_with(storage::COMPLETED_PREFIX) {
                    category = ItemCategory::Completed;
                    prefix_length = storage::COMPLETED_PREFIX.len();
                    let item = Item::new(
                        category,
                        prefix_length,
                        text_in_file.to_string(),
                        index_in_file,
                    );
                    completeds.push(item);
                } else {
                    bail!("unrecognized line prefix")
                }
            }
        }
    }
    items.append(&mut todos);
    items.append(&mut doings);
    items.append(&mut completeds);

    Ok(items)
}

#[derive(Clone)]
struct Item {
    category: ItemCategory,
    prefix_length: usize,
    text_in_file: String,
    index_in_file: usize,
}

impl Item {
    fn new(
        category: ItemCategory,
        prefix_length: usize,
        text_in_file: String,
        index_in_file: usize,
    ) -> Item {
        Item {
            category: category,
            prefix_length: prefix_length,
            text_in_file: text_in_file,
            index_in_file: index_in_file,
        }
    }
}

#[derive(Clone, PartialEq)]
enum ItemCategory {
    Date,
    Todo,
    Doing,
    Completed,
}
