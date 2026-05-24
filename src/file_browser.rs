use std::{fs, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub file_name: String,
}

fn is_config_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ext| matches!(ext.as_str(), "yaml" | "yml" | "json"))
}

pub fn list_config_files(dir: &Path) -> Result<Vec<ConfigFile>> {
    if !dir.exists() {
        bail!("config directory does not exist: {}", dir.display());
    }

    if !dir.is_dir() {
        bail!("config path is not a directory: {}", dir.display());
    }

    let mut files = Vec::new();

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read directory {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() || !is_config_file(&path) {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<invalid utf-8>")
            .to_string();

        files.push(ConfigFile { path, file_name });
    }

    files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(files)
}