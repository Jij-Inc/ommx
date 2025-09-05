use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;
use ocipkg::{oci_spec::image::ImageManifest, ImageName};
use ommx::artifact::{get_image_dir, Artifact};
use std::path::{Path, PathBuf};

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Show the version
    Version,

    /// Login to the remote registry
    Login {
        /// Registry URL, e.g. https://ghcr.io/v2/Jij-Inc/ommx
        registry: String,
        /// Username
        #[clap(short, long)]
        username: Option<String>,
        /// Password
        #[clap(short, long)]
        password: Option<String>,
    },

    /// Show the image manifest as JSON
    Inspect {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },

    /// Push the image to remote registry
    Push {
        /// Path of OCI archive or the container image name stored in local registry
        image_name_or_path: String,
    },

    /// Pull the image from remote registry
    Pull {
        /// Container image name in remote registry
        image_name: String,
    },

    /// Load OCI archive into the local registry
    Load {
        /// Path of OCI archive or OCI directory
        path: PathBuf,
    },

    /// Save the image in the local registry to an OCI archive
    Save {
        /// Container image name
        image_name: String,
        /// Output file name of OCI archive
        output: PathBuf,
    },

    /// List the images in the local registry
    List,

    /// Get the directory where the image is stored
    ImageDirectory {
        /// Container image name
        image_name: String,
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
            let path = get_image_dir(&name);
            if path.exists() {
                return Ok(Self::Local(name));
            } else {
                return Ok(Self::Remote(name));
            }
        }
        bail!("Invalid input: {}", input)
    }

    fn get_manifest(&self) -> Result<ImageManifest> {
        let manifest = match self {
            ImageNameOrPath::OciDir(path) => Artifact::from_oci_dir(path)?.get_manifest()?,
            ImageNameOrPath::OciArchive(path) => {
                Artifact::from_oci_archive(path)?.get_manifest()?
            }
            ImageNameOrPath::Local(name) => {
                let image_dir = get_image_dir(name);
                Artifact::from_oci_dir(&image_dir)?.get_manifest()?
            }
            ImageNameOrPath::Remote(name) => Artifact::from_remote(name.clone())?.get_manifest()?,
        };
        Ok(manifest)
    }
}

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

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
            let mut auth = ocipkg::distribution::StoredAuth::load_all()?;
            match (username, password) {
                (Some(username), Some(password)) => {
                    auth.add(url.domain().unwrap(), username, password);
                }
                (None, None) => {}
                _ => {
                    bail!("--username and --password must be provided at the same time");
                }
            }
            let _token = auth.get_token(&url)?;
            println!("Login succeed");

            auth.save()?;
        }

        Command::Inspect { image_name_or_path } => {
            let manifest = ImageNameOrPath::parse(image_name_or_path)?.get_manifest()?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
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
                let image_dir = get_image_dir(&name);
                let mut artifact = Artifact::from_oci_dir(&image_dir)?;
                artifact.push()?;
            }
            ImageNameOrPath::Remote(name) => {
                bail!("Image not found in local: {}", name)
            }
        },

        Command::Pull { image_name } => {
            let name = ImageName::parse(image_name)?;
            let mut artifact = Artifact::from_remote(name)?;
            artifact.pull()?;
        }

        Command::Save { image_name, output } => {
            let name = ImageName::parse(image_name)?;
            let image_dir = get_image_dir(&name);
            let mut artifact = Artifact::from_oci_dir(&image_dir)?;
            artifact.save(output)?;
        }

        Command::Load { path } => {
            let mut artifact = Artifact::from_oci_archive(path)?;
            artifact.load()?;
        }

        Command::ImageDirectory { image_name } => {
            let name = ImageName::parse(image_name)?;
            let path = get_image_dir(&name);
            println!("{}", path.display());
        }

        Command::List => {
            for image_name in ommx::artifact::get_images()? {
                println!("{image_name}");
            }
        }
    }
    Ok(())
}
