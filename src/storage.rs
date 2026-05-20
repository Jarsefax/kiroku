use color_eyre::{eyre::OptionExt, eyre::Result};
use directories::ProjectDirs;
use rev_lines::RevLines;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
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

pub fn get_data_file_path_buf() -> Result<PathBuf> {
    let data_dir = get_data_dir_path_buf()?;
    let file_path = data_dir.join("default");
    Ok(file_path.to_path_buf())
}

pub fn append_to_data_file(text: String) -> Result<()> {
    let data_dir = get_data_dir_path_buf()?;

    // todo: ask/conf if create or not
    create_data_dir(&data_dir)?;

    // todo: different "files"
    let file_path = data_dir.join("default");

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;

    let ln = format!("{}\n", text);
    file.write_all(ln.as_bytes())?;
    Ok(())
}

fn create_data_dir(data_dir: &PathBuf) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    Ok(())
}

pub fn get_reverse_lines() -> Result<RevLines<File>> {
    let path = get_data_file_path_buf()?;

    let file = File::open(path)?;
    Ok(RevLines::new(file))
}
