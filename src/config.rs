use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    data_dir: Option<String>,
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

pub fn resolve(override_dir: Option<&Path>) -> Result<PathBuf> {
    let home = env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let config_dir = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    resolve_with(override_dir, &home, &config_dir)
}

pub fn resolve_with(
    override_dir: Option<&Path>,
    home: &Path,
    config_dir: &Path,
) -> Result<PathBuf> {
    let selected = if let Some(path) = override_dir {
        path.to_path_buf()
    } else {
        let config_path = config_dir.join("kiroku/config.toml");
        let config = match fs::read_to_string(&config_path) {
            Ok(text) => toml::from_str::<Config>(&text)
                .with_context(|| format!("Invalid configuration in {}", config_path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => {
                return Err(e).with_context(|| format!("Cannot read {}", config_path.display()));
            }
        };
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
