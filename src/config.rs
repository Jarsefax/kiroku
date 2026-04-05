use crate::storage;
use color_eyre::eyre::Result;

pub fn execute() -> Result<String> {
    let data_dir = storage::get_data_dir_path_buf()?;

    Ok(data_dir.to_string_lossy().into_owned())
}
