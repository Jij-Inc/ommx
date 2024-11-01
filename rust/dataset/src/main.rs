use anyhow::Result;
use clap::Parser;
use std::{fs, path::PathBuf};
use zip::ZipArchive;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Convert MIPLIB collections into OMMX Artifact, and Push to GitHub
    Miplib {
        /// Path to downloaded MIPLIB collection.zip file
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let command = Command::parse();
    match command {
        Command::Miplib { path } => {
            println!("Miplib: {:?}", path);
            let f = fs::File::open(path)?;
            let mut ar = ZipArchive::new(f)?;

            for i in 0..ar.len() {
                let file = ar.by_index(i)?;
                println!("Filename: {}", file.name());
            }
        }
    }
    Ok(())
}
