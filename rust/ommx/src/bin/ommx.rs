use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use ocipkg::{image::Image, oci_spec::image::Descriptor};
use ommx::artifact::Artifact;
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

fn show_desc(desc: &Descriptor) {
    println!(" - {}: {}", "Blob".blue().bold(), desc.digest());
    println!("   {}: {}", "Type".blue().bold(), desc.media_type());
    if let Some(annotations) = desc.annotations() {
        println!("   {}:", "Annotations".blue().bold());
        for (key, value) in annotations.iter() {
            println!("     {}: {}", key.bold(), value);
        }
    }
}

fn inspect(path: &Path) -> Result<()> {
    let mut artifact = Artifact::from_oci_archive(path)?;
    let name = artifact
        .get_name()
        .map(|name| name.to_string())
        .unwrap_or("unnamed".to_string());
    println!("{}", format!("[{name}]").bold());
    for (desc, _instance) in artifact.get_instances()? {
        show_desc(&desc);
    }
    for (desc, _solution) in artifact.get_solutions()? {
        show_desc(&desc);
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
