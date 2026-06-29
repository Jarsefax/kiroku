mod append;
mod command;
mod config;
mod interactive;
mod list;
mod rollover;
mod storage;
mod time;

use crate::command::{Cli, ItemCategory, handle_command};
use crate::interactive::app;
use clap::Parser;
use color_eyre::Result;

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
        append::execute(Some(ItemCategory::Doing), text)?;
        Ok(true)
    } else if let Some(command) = args.command {
        handle_command(command)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
