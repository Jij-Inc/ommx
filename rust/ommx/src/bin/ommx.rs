use anyhow::Result;
use clap::{Parser, Subcommand};
use ocipkg::image::Image;
use ommx::artifact::{Artifact, InstanceAnnotations, SolutionAnnotations};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Inspect {
        /// Container image name or the path of OCI archive
        path: PathBuf,
    },
}

fn inspect(path: &Path) -> Result<()> {
    let mut artifact = Artifact::from_oci_archive(path)?;
    let name = artifact
        .get_name()
        .map(|name| name.to_string())
        .unwrap_or("unnamed".to_string());
    println!("[artifact: {name}]");
    for (desc, _instance) in artifact.get_instances()? {
        println!(" - {} ({})", desc.media_type(), desc.digest());
        let annotations = InstanceAnnotations::from_descriptor(&desc);
        for (key, value) in annotations.iter() {
            println!("   - {}: {}", key, value);
        }
    }
    for (desc, _solution) in artifact.get_solutions()? {
        println!(" - {} ({})", desc.media_type(), desc.digest());
        let annotations = SolutionAnnotations::from_descriptor(&desc);
        for (key, value) in annotations.iter() {
            println!("   - {}: {}", key, value);
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Inspect { path } => {
            inspect(path)?;
        }
    }
    Ok(())
}