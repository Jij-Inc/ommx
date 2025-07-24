mod miplib2017;

use anyhow::Result;
use clap::Parser;
use env_logger::{Builder, Env};
use std::path::PathBuf;

/// OMMX Artifact generator for well-known datasets.
///
/// This only support packaging into OMMX Artifact, please use `ommx push` command to upload the artifacts.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// MIPLIB 2017 collections
    Miplib2017 {
        /// Path to downloaded MIPLIB's `collection.zip` or `benchmark.zip` file
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
