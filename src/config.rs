use std::{fmt, fs, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitComponent {
    /// GitHub/Git remote repository URL.
    #[serde(
        alias = "github_remote_repository",
        alias = "github_remote_repo",
        alias = "remote_repository",
        alias = "remote_repo",
        alias = "repo",
        alias = "url"
    )]
    pub remote: String,

    /// Existing local repository copy. If absent, upbuild derives one from save_dir + repo name.
    #[serde(default, alias = "local_copy_dir", alias = "local_copy", alias = "directory")]
    pub local_dir: Option<PathBuf>,

    /// Parent directory where the repository should be cloned if local_dir is absent.
    #[serde(default, alias = "save_directory", alias = "clone_into", alias = "repo_save_dir")]
    pub save_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComponent {
    /// Dockerfile or Compose file path. This is informational unless your command uses it.
    #[serde(default, alias = "dockerfile", alias = "compose_file", alias = "file", alias = "src")]
    pub file_src: Option<PathBuf>,

    /// Full docker/docker-compose command used to build the image.
    #[serde(alias = "docker_build", alias = "build", alias = "image_build_command")]
    pub build_cmd: String,

    /// Full docker/docker-compose command used to deploy/run the container.
    #[serde(alias = "docker_container", alias = "container", alias = "deploy_cmd", alias = "run_cmd")]
    pub container_cmd: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpbuildConfig {
    #[serde(default)]
    pub name: Option<String>,

    pub git: GitComponent,
    pub docker: DockerComponent,
}

impl UpbuildConfig {
    pub fn repo_local_dir(&self) -> PathBuf {
        if let Some(local_dir) = &self.git.local_dir {
            return local_dir.clone();
        }

        let save_dir = self
            .git
            .save_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("repos"));

        save_dir.join(repo_name_from_remote(&self.git.remote))
    }

    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| repo_name_from_remote(&self.git.remote))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Json,
}

impl fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigFormat::Yaml => write!(f, "YAML"),
            ConfigFormat::Json => write!(f, "JSON"),
        }
    }
}

pub fn detect_config_format(path: &Path) -> Result<ConfigFormat> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    match ext.as_str() {
        "yaml" | "yml" => Ok(ConfigFormat::Yaml),
        "json" => Ok(ConfigFormat::Json),
        _ => bail!("unsupported config extension for {}", path.display()),
    }
}

pub fn read_config_file_raw(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read config file {}", path.display()))
}


fn validate_config(cfg: &UpbuildConfig) -> Result<()> {
    if cfg.git.remote.trim().is_empty() {
        bail!("git.remote cannot be empty");
    }

    if cfg.docker.build_cmd.trim().is_empty() {
        bail!("docker.build_cmd cannot be empty");
    }

    if cfg.docker.container_cmd.trim().is_empty() {
        bail!("docker.container_cmd cannot be empty");
    }

    Ok(())
}

pub fn load_config(path: &Path) -> Result<UpbuildConfig> {
    let raw = read_config_file_raw(path)?;
    let format = detect_config_format(path)?;

    let cfg = match format {
        ConfigFormat::Yaml => yaml_serde::from_str::<UpbuildConfig>(&raw)
            .with_context(|| format!("failed to parse YAML config {}", path.display()))?,
        ConfigFormat::Json => serde_json::from_str::<UpbuildConfig>(&raw)
            .with_context(|| format!("failed to parse JSON config {}", path.display()))?,
    };

    validate_config(&cfg)?;
    Ok(cfg)
}

fn repo_name_from_remote(remote: &str) -> String {
    let trimmed = remote.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("repo");

    last.strip_suffix(".git").unwrap_or(last).to_string()
}

