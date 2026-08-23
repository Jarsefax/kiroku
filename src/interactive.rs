use crate::storage;
use color_eyre::{Result, eyre::bail};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub struct App {
    items: Vec<Item>,
    state: ListState,
    edit: Option<Item>,
    input: String,
    input_character_index: usize,
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
            input: String::new(),
            input_character_index: 0,
        })
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match self.edit {
                    None => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => self.up(),
                        KeyCode::Down | KeyCode::Char('j') => self.down(),
                        KeyCode::Char('e') => self.enter_edit(),
                        _ => {}
                    },
                    Some(_) => match key.code {
                        KeyCode::Esc => self.back(),
                        KeyCode::Enter => {
                            _ = self.exit_edit()?;
                        }
                        KeyCode::Char(to_insert) => self.add_to_input(to_insert),
                        KeyCode::Left => self.left(),
                        KeyCode::Right => self.right(),
                        KeyCode::Delete => self.forward_delete_from_input(),
                        KeyCode::Backspace => self.backward_delete_from_input(),
                        _ => {}
                    },
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

    fn enter_edit(&mut self) {
        if self.items.is_empty() {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                let item = self.items[i].clone();
                let text = &item.text_in_file;
                let len = item.prefix_length;
                let input = &text[len..];
                self.input = input.to_string();
                self.edit = Some(item);
            }
            None => {
                return;
            }
        }
    }

    fn exit_edit(&mut self) -> Result<()> {
        let edit = self.edit.as_ref().unwrap();
        let res: ();
        if self.input != edit.text_in_file {
            res = storage::replace_line(edit.index_in_file, &self.input)?;
            // todo replace text on screen
        } else {
            res = ();
        }

        self.edit = None;
        Ok(res)
    }

    fn back(&mut self) {
        if let Some(_) = self.edit {
            self.edit = None;
        }
    }

    fn clamp_character_index(&self, new_character_index: usize) -> usize {
        new_character_index.clamp(0, self.input.chars().count())
    }

    fn right(&mut self) {
        let caret_moved_right = self.input_character_index.saturating_add(1);
        self.input_character_index = self.clamp_character_index(caret_moved_right);
    }

    fn left(&mut self) {
        let caret_moved_left = self.input_character_index.saturating_sub(1);
        self.input_character_index = self.clamp_character_index(caret_moved_left);
    }

    fn input_byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.input_character_index)
            .unwrap_or(self.input.len())
    }

    fn add_to_input(&mut self, new_character: char) {
        let i = self.input_byte_index();
        self.input.insert(i, new_character);
    }

    fn backward_delete_from_input(&mut self) {
        let caret_is_not_in_first_position = self.input_character_index != 0;
        if caret_is_not_in_first_position {
            let target_char_index = self.input_character_index - 1;
            if let Some((byte_offset, _)) = self.input.char_indices().nth(target_char_index) {
                self.input.remove(byte_offset);
                self.left();
            }
        }
    }

    fn forward_delete_from_input(&mut self) {
        let caret_is_not_in_last_position = self.input_character_index < self.input.chars().count();
        if caret_is_not_in_last_position {
            let target_char_index = self.input_character_index;
            if let Some((byte_offset, _)) = self.input.char_indices().nth(target_char_index) {
                self.input.remove(byte_offset);
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

        if let Some(_) = &mut self.edit {
            let center = area.centered(Constraint::Percentage(60), Constraint::Percentage(20));
            frame.render_widget(Clear, center);

            let popup = Block::bordered().title(" Edit ");
            let paragraph = Paragraph::new(self.input.clone()).block(popup);
            frame.render_widget(paragraph, center);

            let pos = Position::new(
                center.x + self.input_character_index as u16 + 1,
                center.y + 1,
            );
            frame.set_cursor_position(pos);
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
