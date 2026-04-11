use crate::{command::ItemCategory, storage};
use color_eyre::eyre::Result;

pub fn execute(category: Option<ItemCategory>, text: String) -> Result<()> {
    let mut cat = ItemCategory::Doing;
    if let Some(c) = category {
        cat = c
    }

    let t = match cat {
        ItemCategory::Todo => "todo: ".to_owned() + &text,
        ItemCategory::Doing => "doing: ".to_owned() + &text,
        ItemCategory::Completed => "completed: ".to_owned() + &text,
    };
    storage::append_to_data_file(t)
}
