use crate::{command::ItemCategory, storage};
use clap::Parser;
use color_eyre::eyre::Result;

#[derive(Parser)]
#[command(visible_alias = "a")]
pub struct Cmd {
    /// Category to append to (default is ´doing´)
    #[arg(value_enum, short, long, visible_alias = "cat")]
    pub category: Option<ItemCategory>,

    ///Text to append
    pub text: String,
}

pub fn execute(category: Option<ItemCategory>, text: String) -> Result<()> {
    let mut cat = ItemCategory::Doing;
    if let Some(c) = category {
        cat = c
    }

    let t = match cat {
        ItemCategory::Todo => storage::TODO_PREFIX.to_owned() + &text,
        ItemCategory::Doing => storage::DOING_PREFIX.to_owned() + &text,
        ItemCategory::Completed => storage::COMPLETED_PREFIX.to_owned() + &text,
    };
    storage::append_to_data_file(&t)
}
