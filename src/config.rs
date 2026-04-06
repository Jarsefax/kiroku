use crate::storage;
use color_eyre::eyre::Result;

pub fn execute(file: bool, directory: bool) -> Result<String> {
    let mut include_file = file;
    if file == false && directory == false {
        include_file = true;
    }

    let res = match include_file {
        false => storage::get_data_dir_path_buf()?,
        true => storage::get_data_file_path_buf()?,
    };
    Ok(res.to_string_lossy().into_owned())
}
