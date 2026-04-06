use crate::storage;
use color_eyre::eyre::Result;
use std::fs;

pub fn execute() -> Result<String> {
    let path = storage::get_data_file_path_buf()?;
    let res = fs::read_to_string(path)?;
    Ok(res)
}
