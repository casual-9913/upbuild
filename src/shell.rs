use std::{path::Path, process::Command};

use anyhow::{bail, Context, Result};

pub fn run_shell_command(command: &str, working_dir: Option<&Path>) -> Result<()> {
    let command = command.trim();
    if command.is_empty() {
        bail!("cannot run an empty command");
    }

    println!("\n$ {command}");

    let mut process = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };

    if let Some(dir) = working_dir {
        process.current_dir(dir);
    }

    let status = process
        .status()
        .with_context(|| format!("failed to execute shell command: {command}"))?;

    if !status.success() {
        bail!("command failed with status {status}: {command}");
    }

    Ok(())
}
