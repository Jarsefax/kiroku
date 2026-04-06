mod append;
mod command;
mod config;
mod storage;

use crate::command::{Cli, Commands};
use clap::Parser;
use color_eyre::Result;
use ratatui::{DefaultTerminal, Frame};

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();
    if handle_args(args)? {
        Ok(())
    } else {
        ratatui::run(app)?;
        Ok(())
    }
}

fn handle_args(args: Cli) -> Result<bool> {
    if let Some(text) = args.text {
        append::execute(text)?;
        Ok(true)
    } else if let Some(command) = args.command {
        handle_command(command)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Append { text } => append::execute(text),
        Commands::Config { file, directory } => {
            let res = config::execute(file, directory)?;
            println!("{}", res);
            return Ok(());
        }
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
