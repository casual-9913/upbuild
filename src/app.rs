use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use inquire::{Confirm, Select};

use crate::{
    cli::Cli,
    config::{load_config, read_config_file_raw, UpbuildConfig},
    docker_ops::{docker_build, docker_container},
    file_browser::{list_config_files, ConfigFile},
    git_ops::{repo_clone, repo_status, repo_update},
    settings,
};

enum ConfigAction {
    CloneRepository,
    RepositoryStatus,
    UpdateRepository,
    BuildImage,
    DeployContainer,
    RunAll,
    Back,
    Quit,
}

impl std::fmt::Display for ConfigAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            ConfigAction::CloneRepository => "Clone Repository",
            ConfigAction::RepositoryStatus => "Repository Status",
            ConfigAction::UpdateRepository => "Update Repository",
            ConfigAction::BuildImage => "Build Image",
            ConfigAction::DeployContainer => "Deploy Container",
            ConfigAction::RunAll => "Run All",
            ConfigAction::Back => "Back to Config List",
            ConfigAction::Quit => "Quit",
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileMenuItem {
    Config(ConfigFile),
    Quit,
}

impl std::fmt::Display for FileMenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileMenuItem::Config(file) => write!(f, "{}", file.file_name),
            FileMenuItem::Quit => write!(f, "Quit"),
        }
    }
}

pub fn run(cli: Cli) -> Result<()> {
    if let Some(dir) = cli.set_default_dir {
        settings::set_default_dir(&dir)?;
        println!("Default config directory set to: {}", dir.display());
        return Ok(());
    }

    let config_dir = resolve_config_dir(cli.dir)?;
    run_interactive(&config_dir)
}

fn resolve_config_dir(cli_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = cli_dir {
        return Ok(dir);
    }

    settings::get_default_dir()?.context(
        "no config directory provided; use `upbuild -d <dir>` or `upbuild --set_default_dir <dir>`",
    )
}

fn run_interactive(config_dir: &Path) -> Result<()> {
    loop {
        let selected = select_config_file(config_dir)?;

        let Some(config_file) = selected else {
            println!("Exiting upbuild.");
            return Ok(());
        };

        let loaded = preview_and_confirm_config(&config_file.path)?;
        if !loaded {
            continue;
        }

        let cfg = load_config(&config_file.path)?;
        run_config_menu(&cfg)?;
    }
}

fn select_config_file(config_dir: &Path) -> Result<Option<ConfigFile>> {
    let files = list_config_files(config_dir)?;

    if files.is_empty() {
        bail!(
            "no YAML or JSON config files found in {}",
            config_dir.display()
        );
    }

    let mut items: Vec<FileMenuItem> = files.into_iter().map(FileMenuItem::Config).collect();
    items.push(FileMenuItem::Quit);

    println!("\nConfig directory: {}", config_dir.display());

    let selected = Select::new("Choose a config file:", items)
        .prompt()
        .context("failed to read file selection")?;

    match selected {
        FileMenuItem::Config(file) => Ok(Some(file)),
        FileMenuItem::Quit => Ok(None),
    }
}

fn preview_and_confirm_config(path: &Path) -> Result<bool> {
    let raw = read_config_file_raw(path)?;

    println!("\n================ CONFIG PREVIEW ================");
    println!("File: {}", path.display());
    println!("------------------------------------------------");
    println!("{raw}");
    println!("================================================\n");

    let load = Confirm::new("Load this config?")
        .with_default(true)
        .prompt()
        .context("failed to read confirmation")?;

    Ok(load)
}

fn run_config_menu(cfg: &UpbuildConfig) -> Result<()> {
    loop {
        println!("\nLoaded config: {}", cfg.display_name());
        println!("Local repository path: {}", cfg.repo_local_dir().display());

        let action = Select::new(
            "Choose an action:",
            vec![
                ConfigAction::CloneRepository,
                ConfigAction::RepositoryStatus,
                ConfigAction::UpdateRepository,
                ConfigAction::BuildImage,
                ConfigAction::DeployContainer,
                ConfigAction::RunAll,
                ConfigAction::Back,
                ConfigAction::Quit,
            ],
        )
        .prompt()
        .context("failed to read action selection")?;

        match action {
            ConfigAction::CloneRepository => repo_clone(cfg)?,
            ConfigAction::RepositoryStatus => {
                repo_status(cfg)?;
            }
            ConfigAction::UpdateRepository => repo_update(cfg)?,
            ConfigAction::BuildImage => docker_build(cfg)?,
            ConfigAction::DeployContainer => docker_container(cfg)?,
            ConfigAction::RunAll => update_n_build(cfg)?,
            ConfigAction::Back => return Ok(()),
            ConfigAction::Quit => std::process::exit(0),
        }
    }
}

pub fn update_n_build(cfg: &UpbuildConfig) -> Result<()> {
    println!("\n[1/4] Clone Repository if missing");
    repo_clone(cfg)?;

    println!("\n[2/4] Update Repository if outdated");
    repo_update(cfg)?;

    println!("\n[3/4] Build Image");
    docker_build(cfg)?;

    println!("\n[4/4] Deploy Container");
    docker_container(cfg)?;

    println!("\nRun All complete.");
    Ok(())
}
