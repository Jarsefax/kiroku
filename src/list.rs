use crate::storage;
use color_eyre::eyre::Result;
use rev_lines::RevLines;
use std::fs::File;

pub fn execute() -> Result<String> {
    let path = storage::get_data_file_path_buf()?;

    let file = File::open(path)?;
    let rev_lines = RevLines::new(file);

    let mut traversed_lines = Vec::new();
    for rev_line in rev_lines {
        if let Ok(line) = rev_line {
            let matched = line.starts_with('-');
            traversed_lines.push(line);
            if matched {
                break;
            }
        }
    }
    traversed_lines.reverse();

    Ok(traversed_lines.join("\n"))
}
