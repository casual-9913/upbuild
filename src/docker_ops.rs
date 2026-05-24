use anyhow::Result;

use crate::{config::UpbuildConfig, shell::run_shell_command};

pub fn docker_build(cfg: &UpbuildConfig) -> Result<()> {
    let local_dir = cfg.repo_local_dir();

    if let Some(file_src) = &cfg.docker.file_src {
        println!("Docker file source: {}", file_src.display());
    }

    run_shell_command(&cfg.docker.build_cmd, Some(&local_dir))
}

pub fn docker_container(cfg: &UpbuildConfig) -> Result<()> {
    let local_dir = cfg.repo_local_dir();
    run_shell_command(&cfg.docker.container_cmd, Some(&local_dir))
}
