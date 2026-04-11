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
    #[command(visible_alias = "a")]
    Append {
        /// Category to append to (default is ´doing´)
        #[arg(short, long, value_enum)]
        category: Option<ItemCategory>,

        ///Text to append
        text: String,
    },

    /// Lists content of file
    #[command(visible_aliases = ["l", "ls"])]
    List {},

    /// Show and set configuration
    #[command(visible_aliases = ["c", "cfg"])]
    Config {
        /// Display data file path
        #[arg(short, long)]
        file: bool,

        /// Display data directory path
        #[arg(short, long)]
        directory: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ItemCategory {
    /// Task to do
    #[value(alias = "t")]
    Todo,

    /// Task being worked on
    #[value(alias = "d")]
    Doing,

    /// Completed task
    #[value(alias = "c")]
    Completed,
}

pub fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Append { category, text } => append::execute(category, text),
        Commands::List {} => {
            let res = list::execute()?;
            println!("{}", res);
            return Ok(());
        }
        Commands::Config { file, directory } => {
            let res = config::execute(file, directory)?;
            println!("{}", res);
            return Ok(());
        }
    }
}
