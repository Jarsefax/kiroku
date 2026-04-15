use crate::storage;
use clap::Parser;
use color_eyre::eyre::Result;

#[derive(Parser)]
#[command(visible_aliases = ["c", "cfg"])]
pub struct Cmd {
    /// Display data file path
    #[arg(short, long)]
    file: bool,

    /// Display data directory path
    #[arg(short, long, visible_alias = "dir")]
    directory: bool,
}

pub fn execute(cmd: Cmd) -> Result<String> {
    let mut include_file = cmd.file;
    if cmd.file == false && cmd.directory == false {
        include_file = true;
    }

    let res = match include_file {
        false => storage::get_data_dir_path_buf()?,
        true => storage::get_data_file_path_buf()?,
    };
    Ok(res.to_string_lossy().into_owned())
}
