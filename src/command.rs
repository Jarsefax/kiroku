use crate::{append, config, list};
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Text to add
    pub text: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Appends text to file
    Append(append::Cmd),

    /// Lists content of file, default behaviour is listing last recorded day
    List(list::Cmd),

    /// Show and set configuration
    Config(config::Cmd),
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ItemCategory {
    /// Task to do
    #[value(aliases = ["t", "td"])]
    Todo,

    /// Task being worked on
    #[value(aliases = ["d", "do"])]
    Doing,

    /// Completed task
    #[value(aliases = ["c", "done"])]
    Completed,
}

pub fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Append(cmd) => append::execute(cmd.category, cmd.text),
        Commands::List(cmd) => {
            let res = list::execute(cmd)?;
            println!("{}", res);
            return Ok(());
        }
        Commands::Config(cmd) => {
            let res = config::execute(cmd)?;
            println!("{}", res);
            return Ok(());
        }
    }
}
