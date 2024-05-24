use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use clap::Parser;
use colored::Colorize;
use ocipkg::{image::Image, oci_spec::image::Descriptor, ImageName};
use ommx::artifact::{image_dir, Artifact};
use std::path::{Path, PathBuf};

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    Version,
    Login {
        /// Registry URL, e.g. https://ghcr.io/v2/Jij-Inc/ommx
        registry: String,
        /// Username
        #[clap(short, long)]
        username: String,
        /// Password
        #[clap(short, long)]
        password: String,
    },
    Inspect {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },
    Push {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },
}

enum ImageNameOrPath {
    Local(ImageName),
    Remote(ImageName),
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
            let path = image_dir(&name)?;
            if path.exists() {
                return Ok(Self::Local(name));
            } else {
                return Ok(Self::Remote(name));
            }
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
        Command::Version => {
            println!(
                "{:>12} {}",
                "Version".blue().bold(),
                built_info::PKG_VERSION,
            );
            println!("{:>12} {}", "Target".blue().bold(), built_info::TARGET,);
            if let Some(hash) = built_info::GIT_COMMIT_HASH {
                println!("{:>12} {}", "Git Commit".blue().bold(), hash);
            }
        }
        Command::Login {
            registry,
            username,
            password,
        } => {
            let url = url::Url::parse(registry)?;
            let octet = STANDARD.encode(format!("{}:{}", username, password,));
            let mut new_auth = ocipkg::distribution::StoredAuth::default();
            new_auth.insert(url.domain().unwrap(), octet);
            let _token = new_auth.get_token(&url)?;
            println!("Login succeed");

            let mut auth = ocipkg::distribution::StoredAuth::load()?;
            auth.append(new_auth)?;
            auth.save()?;
        }

        Command::Inspect { image_name_or_path } => {
            match ImageNameOrPath::parse(image_name_or_path)? {
                ImageNameOrPath::OciDir(path) => {
                    let artifact = Artifact::from_oci_dir(&path)?;
                    inspect(artifact)?;
                }
                ImageNameOrPath::OciArchive(path) => {
                    let artifact = Artifact::from_oci_archive(&path)?;
                    inspect(artifact)?;
                }
                ImageNameOrPath::Local(name) => {
                    let image_dir = image_dir(&name)?;
                    let artifact = Artifact::from_oci_dir(&image_dir)?;
                    inspect(artifact)?;
                }
                ImageNameOrPath::Remote(name) => {
                    let artifact = Artifact::from_remote(name)?;
                    inspect(artifact)?;
                }
            }
        }

        Command::Push { image_name_or_path } => match ImageNameOrPath::parse(image_name_or_path)? {
            ImageNameOrPath::OciDir(path) => {
                let mut artifact = Artifact::from_oci_dir(&path)?;
                artifact.push()?;
            }
            ImageNameOrPath::OciArchive(path) => {
                let mut artifact = Artifact::from_oci_archive(&path)?;
                artifact.push()?;
            }
            ImageNameOrPath::Local(name) => {
                let image_dir = image_dir(&name)?;
                let mut artifact = Artifact::from_oci_dir(&image_dir)?;
                artifact.push()?;
            }
            ImageNameOrPath::Remote(name) => {
                bail!("Image not found in local: {}", name)
            }
        },
    }
    Ok(())
}
