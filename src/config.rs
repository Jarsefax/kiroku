use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate, Weekday};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    data_dir: Option<String>,
    #[serde(default)]
    recurring_todos: Vec<RecurringTodo>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecurringTodo {
    title: String,
    days: Vec<DayOfWeek>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl DayOfWeek {
    fn weekday(self) -> Weekday {
        match self {
            Self::Monday => Weekday::Mon,
            Self::Tuesday => Weekday::Tue,
            Self::Wednesday => Weekday::Wed,
            Self::Thursday => Weekday::Thu,
            Self::Friday => Weekday::Fri,
            Self::Saturday => Weekday::Sat,
            Self::Sunday => Weekday::Sun,
        }
    }
}

impl RecurringTodo {
    pub(crate) fn validate(&mut self) -> Result<()> {
        self.title = crate::storage::validate_title(&self.title)
            .context("Invalid recurring todo title")?
            .to_owned();
        if self.days.is_empty() {
            bail!(
                "Recurring todo {:?} must specify at least one weekday in days",
                self.title
            );
        }
        Ok(())
    }

    pub(crate) fn title_on(&self, date: NaiveDate) -> Option<&str> {
        self.days
            .iter()
            .any(|day| day.weekday() == date.weekday())
            .then_some(self.title.as_str())
    }
}

fn read_config(config_dir: &Path) -> Result<Config> {
    let config_path = config_dir.join("kiroku/config.toml");
    match fs::read_to_string(&config_path) {
        Ok(text) => toml::from_str::<Config>(&text)
            .with_context(|| format!("Invalid configuration in {}", config_path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).with_context(|| format!("Cannot read {}", config_path.display())),
    }
}

pub fn expand_home(path: &Path, home: &Path) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" {
        Ok(home.to_path_buf())
    } else if let Some(rest) = text.strip_prefix("~/") {
        Ok(home.join(rest))
    } else if text.starts_with('~') {
        bail!("Use ~/ for home-relative paths; ~user paths are not supported")
    } else {
        Ok(path.to_path_buf())
    }
}

fn locations() -> Result<(PathBuf, PathBuf)> {
    let home = env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let config_dir = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    Ok((home, config_dir))
}

pub fn resolve(override_dir: Option<&Path>) -> Result<PathBuf> {
    let (home, config_dir) = locations()?;
    resolve_with(override_dir, &home, &config_dir)
}

pub fn recurring_todos() -> Result<Vec<RecurringTodo>> {
    let (_, config_dir) = locations()?;
    recurring_todos_with(&config_dir)
}

pub fn recurring_todos_with(config_dir: &Path) -> Result<Vec<RecurringTodo>> {
    let mut todos = read_config(config_dir)?.recurring_todos;
    for (index, todo) in todos.iter_mut().enumerate() {
        todo.validate().with_context(|| {
            format!(
                "Invalid recurring_todos entry {} in {}",
                index + 1,
                config_dir.join("kiroku/config.toml").display()
            )
        })?;
    }
    Ok(todos)
}

pub fn resolve_with(
    override_dir: Option<&Path>,
    home: &Path,
    config_dir: &Path,
) -> Result<PathBuf> {
    let selected = if let Some(path) = override_dir {
        path.to_path_buf()
    } else {
        let config = read_config(config_dir)?;
        match config.data_dir {
            Some(value) if value.trim().is_empty() => bail!("data_dir cannot be empty"),
            Some(value) => PathBuf::from(value),
            None => home.join("Documents/kiroku"),
        }
    };
    let selected = expand_home(&selected, home)?;
    if selected.as_os_str().is_empty() {
        bail!("Data directory cannot be empty");
    }
    Ok(if selected.is_absolute() {
        selected
    } else {
        env::current_dir()?.join(selected)
    })
}
