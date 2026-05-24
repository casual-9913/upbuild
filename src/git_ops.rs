
use std::{fs, path::{Path, PathBuf}, process::Command};

use anyhow::{bail, Context, Result};

use crate::config::UpbuildConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoState {
    Missing,
    Updated,
    Outdated,
}


pub fn repo_clone(cfg: &UpbuildConfig) -> Result<()> {
    let local_dir = cfg.repo_local_dir();

    if is_git_repo(&local_dir) {
        println!("Repository already exists: {}", local_dir.display());
        return Ok(());
    }

    if local_dir.exists() {
        bail!(
            "target path exists but is not a Git repository: {}",
            local_dir.display()
        );
    }

    if let Some(parent) = local_dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create clone parent directory {}", parent.display()))?;
    }

    println!("Cloning {} into {}", cfg.git.remote, local_dir.display());
    run_git(None, &["clone", cfg.git.remote.as_str(), path_to_str(&local_dir)?])?;
    Ok(())
}

pub fn repo_status(cfg: &UpbuildConfig) -> Result<RepoState> {
    let local_dir = cfg.repo_local_dir();

    if !is_git_repo(&local_dir) {
        println!("Repository missing: {}", local_dir.display());
        return Ok(RepoState::Missing);
    }

    ensure_remote(&local_dir, &cfg.git.remote)?;
    run_git(Some(&local_dir), &["fetch", "--prune"])?;

    let upstream = current_upstream(&local_dir)?;
    let local = git_output(&local_dir, &["rev-parse", "HEAD"])?;
    let remote = git_output(&local_dir, &["rev-parse", upstream.as_str()])?;

    if local.trim() == remote.trim() {
        println!("Repository Status: Updated");
        Ok(RepoState::Updated)
    } else {
        println!("Repository Status: Outdated");
        println!("local : {}", local.trim());
        println!("remote: {}", remote.trim());
        Ok(RepoState::Outdated)
    }
}

pub fn repo_update(cfg: &UpbuildConfig) -> Result<()> {
    let local_dir = cfg.repo_local_dir();

    match repo_status(cfg)? {
        RepoState::Missing => {
            println!("Repository missing. Cloning first.");
            repo_clone(cfg)?;
        }
        RepoState::Updated => {
            println!("No update needed.");
        }
        RepoState::Outdated => {
            let upstream = current_upstream(&local_dir)?;
            println!("Updating repository from {upstream}");
            run_git(Some(&local_dir), &["pull", "--ff-only"])?;
        }
    }

    Ok(())
}



fn path_to_str(path: &PathBuf) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}


fn run_git(local_dir: Option<&Path>, args: &[&str]) -> Result<()> {
    println!("$ git {}", args.join(" "));

    let mut command = Command::new("git");
    command.args(args);

    if let Some(local_dir) = local_dir {
        command.current_dir(local_dir);
    }

    let status = command
        .status()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if !status.success() {
        bail!("git {} failed with status {status}", args.join(" "));
    }

    Ok(())
}

fn git_output(local_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(local_dir)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn ensure_remote(local_dir: &Path, expected_remote: &str) -> Result<()> {
    let actual = git_output(local_dir, &["remote", "get-url", "origin"])
        .context("failed to read origin remote URL")?;

    if actual.trim() != expected_remote.trim() {
        println!(
            "Warning: config remote differs from repository origin.\n  config: {}\n  origin: {}",
            expected_remote,
            actual.trim()
        );
    }

    Ok(())
}

fn current_upstream(local_dir: &Path) -> Result<String> {
    let upstream = git_output(local_dir, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .context("no upstream branch configured; run `git branch --set-upstream-to origin/<branch>`")?;
    Ok(upstream.trim().to_string())
}