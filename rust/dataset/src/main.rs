mod miplib2017;

use anyhow::Result;
use clap::Parser;
use env_logger::{Builder, Env};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Convert MIPLIB collections into OMMX Artifact, and Push to GitHub
    Miplib2017 {
        /// Path to downloaded MIPLIB collection.zip file
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    Builder::from_env(Env::default().default_filter_or("info")).init();

    let command = Command::parse();
    match command {
        Command::Miplib2017 { path } => {
            miplib2017::package(&path)?;
        }
    }
    Ok(())
}
