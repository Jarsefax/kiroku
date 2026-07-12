use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{DefaultTerminal, Frame};

// enum Mode {
//     Normal,
//     Editing,
// }

pub struct App {}

impl App {
    pub fn new() -> App {
        App {}
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Press 'q' to break the loop and exit safely
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        frame.render_widget("hello world", frame.area());
    }
}
