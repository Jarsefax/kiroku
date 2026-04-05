use crate::storage;
use color_eyre::eyre::Result;

pub fn execute(text: String) -> Result<()> {
    storage::append_to_data_file(text)
}
