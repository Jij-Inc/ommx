mod miplib2017;
mod qplib;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

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
    /// QPLIB collections
    Qplib {
        /// Path to downloaded QPLIB's zip file containing `*.qplib` files
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let command = Command::parse();
    match command {
        Command::Miplib2017 { path } => {
            miplib2017::package(&path)?;
        }
        Command::Qplib { path } => {
            qplib::package(&path)?;
        }
    }
    Ok(())
}
