use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Convert MIPLIB collections into OMMX Artifact, and Push to GitHub
    Miplib {
        /// Path to downloaded MIPLIB collection.zip file
        path: PathBuf,
    },
}

fn main() {
    let command = Command::parse();
    dbg!(&command);
}