use crate::{append, config, list};
use clap::{Parser, Subcommand};
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
    Append {
        ///Text to append
        text: String,
    },
    /// Lists content of file
    List {},
    Config {
        /// Display data file path
        #[arg(short, long)]
        file: bool,

        /// Display data directory path
        #[arg(short, long)]
        directory: bool,
    },
}

pub fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Append { text } => append::execute(text),
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
