use clap::Parser;
use color_eyre::eyre::OptionExt;
use directories::ProjectDirs;
use ratatui::{DefaultTerminal, Frame};
use std::fs;

#[derive(Parser)]
struct Cli {
    text: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    if let Some(text) = args.text {
        const QUALIFIER: &str = "io.github.jarsefax.kiroku";
        const ORGANIZATION: &str = "jarsefax";
        const APPLICATION: &str = "Kiroku";
        const PROJECT_DIRECTORIES_ERROR_MESSAGE: &str = "could not find storage directory";
        let proj_dirs =
            ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
                .ok_or_eyre(PROJECT_DIRECTORIES_ERROR_MESSAGE)?;
        let data_dir = proj_dirs.data_dir();
        println!("dir: {:?}", data_dir);

        // todo: ask/conf if create or not
        fs::create_dir_all(data_dir)?;

        // todo: different "files"
        let file_path = data_dir.join("default");
        println!("file: {:?}", file_path);
        fs::write(&file_path, text)?;

        Ok(())
    } else {
        ratatui::run(app)?;
        Ok(())
    }
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(())
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("hello world", frame.area());
}