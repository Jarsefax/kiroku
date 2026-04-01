use crate::storage;
use color_eyre::eyre::Result;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

pub fn execute(text: String) -> Result<()> {
    let data_dir = storage::get_data_dir()?;

    // todo: ask/conf if create or not
    fs::create_dir_all(&data_dir)?;

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
