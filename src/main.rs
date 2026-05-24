mod app;
mod cli;
mod config;
mod docker_ops;
mod file_browser;
mod git_ops;
mod settings;
mod shell;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    app::run(cli)
}
