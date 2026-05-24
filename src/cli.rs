use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "upbuild")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short = 'd', long = "dir", value_name = "DIR")]
    pub dir: Option<PathBuf>,

    #[arg(long = "set_default_dir", value_name = "DIR")]
    pub set_default_dir: Option<PathBuf>,
}