mod storage;

use clap::Parser;
use color_eyre::Result;
use ratatui::{DefaultTerminal, Frame};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Text att lägga till
    text: Option<String>,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    if let Some(text) = args.text {
        let data_dir = storage::get_data_dir()?;

        // todo: ask/conf if create or not
        fs::create_dir_all(&data_dir)?;

        // todo: different "files"
        let file_path = data_dir.join("default");

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(file_path)
            .expect("Failed to open the file");

        let ln = format!("{}\n", text);
        file.write_all(ln.as_bytes())
            .expect("Failed to append to the file");
        Ok(())
    } else {
        ratatui::run(app)?;
        Ok(())
    }
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("hello world", frame.area());
}
