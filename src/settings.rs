use std::{fs, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub default_dir: Option<PathBuf>,
}

fn settings_file_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine user config directory")?;
    Ok(base.join("upbuild").join("settings.json"))
}

pub fn set_default_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        bail!("default directory does not exist: {}", dir.display());
    }

    if !dir.is_dir() {
        bail!("default path is not a directory: {}", dir.display());
    }

    let settings = Settings {
        default_dir: Some(dir.to_path_buf()),
    };

    let path = settings_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create settings directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(&settings)?;
    fs::write(&path, raw).with_context(|| format!("failed to write settings file {}", path.display()))?;

    Ok(())
}

pub fn get_default_dir() -> Result<Option<PathBuf>> {
    let path = settings_file_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read settings file {}", path.display()))?;

    let settings: Settings = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse settings file {}", path.display()))?;

    Ok(settings.default_dir)
}