use crate::{ConfigLevels, storage};
use color_eyre::eyre::Result;

pub fn execute(level: Option<ConfigLevels>) -> Result<String> {
    let mut l = ConfigLevels::File;
    if let Some(input) = level {
        l = input;
    }

    let res = match l {
        ConfigLevels::Directory => storage::get_data_dir_path_buf()?,
        ConfigLevels::File => storage::get_data_file_path_buf()?,
    };
    Ok(res.to_string_lossy().into_owned())
}
