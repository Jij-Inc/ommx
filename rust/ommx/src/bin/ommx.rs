use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;
use ocipkg::{image::Image, oci_spec::image::Descriptor, ImageName};
use ommx::artifact::{image_dir, Artifact};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    Inspect {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },
}

enum ImageNameOrPath {
    ImageName(ImageName),
    OciArchive(PathBuf),
    OciDir(PathBuf),
}

impl ImageNameOrPath {
    fn parse(input: &str) -> Result<Self> {
        let path: &Path = input.as_ref();
        if path.is_dir() {
            return Ok(Self::OciDir(path.to_path_buf()));
        }
        if path.is_file() {
            return Ok(Self::OciArchive(path.to_path_buf()));
        }
        if let Ok(name) = ImageName::parse(input) {
            return Ok(Self::ImageName(name));
        }
        bail!("Invalid input: {}", input)
    }
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

fn inspect<Base: Image>(mut artifact: Artifact<Base>) -> Result<()> {
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
    let command = Command::parse();
    match &command {
        Command::Inspect { image_name_or_path } => {
            match ImageNameOrPath::parse(&image_name_or_path)? {
                ImageNameOrPath::OciDir(path) => {
                    let artifact = Artifact::from_oci_dir(&path)?;
                    inspect(artifact)?;
                }
                ImageNameOrPath::OciArchive(path) => {
                    let artifact = Artifact::from_oci_archive(&path)?;
                    inspect(artifact)?;
                }
                ImageNameOrPath::ImageName(name) => {
                    let image_dir = image_dir(&name)?;
                    if image_dir.exists() {
                        let artifact = Artifact::from_oci_dir(&image_dir)?;
                        inspect(artifact)?;
                    } else {
                        let artifact = Artifact::from_remote(name)?;
                        inspect(artifact)?;
                    }
                }
            }
        }
    }
    Ok(())
}
