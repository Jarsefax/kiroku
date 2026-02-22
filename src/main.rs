use clap::Parser;
use ratatui::{DefaultTerminal, Frame};

#[derive(Parser)]
struct Cli {
    name: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    if let Some(name) = args.name {
        println!("name: {}", name);
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
            break Ok(())
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("hello world", frame.area());
}