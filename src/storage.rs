use color_eyre::{eyre::OptionExt, eyre::Result};
use directories::ProjectDirs;
use std::path::PathBuf;

const QUALIFIER: &str = "io.github.jarsefax.kiroku";
const ORGANIZATION: &str = "jarsefax";
const APPLICATION: &str = "Kiroku";
const PROJECT_DIRECTORIES_ERROR_MESSAGE: &str = "could not find storage directory";

pub fn get_data_dir_path_buf() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_eyre(PROJECT_DIRECTORIES_ERROR_MESSAGE)?;
    let data_dir = proj_dirs.data_dir();

    Ok(data_dir.to_path_buf())
}
