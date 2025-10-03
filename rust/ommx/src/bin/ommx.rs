use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;
use ocipkg::{oci_spec::image::ImageManifest, ImageName};
use ommx::artifact::{get_local_registry_path, Artifact};
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

    /// Get the base path for an image in the local registry
    LocalRegistryPath {
        /// Container image name
        image_name: String,
    },

    /// Get the directory where the image is stored (deprecated: use local-registry-path instead)
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
            // Check for both oci-archive and oci-dir formats
            let base_path = get_local_registry_path(&name);
            let archive_path = base_path.with_extension("ommx");

            // Priority: oci-archive > oci-dir
            if archive_path.exists() && archive_path.is_file() {
                return Ok(Self::Local(name));
            }
            if base_path.exists() && base_path.is_dir() {
                return Ok(Self::Local(name));
            }
            return Ok(Self::Remote(name));
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
                // Check for both formats, prioritizing oci-archive
                let base_path = get_local_registry_path(name);
                let archive_path = base_path.with_extension("ommx");

                if archive_path.exists() && archive_path.is_file() {
                    Artifact::from_oci_archive(&archive_path)?.get_manifest()?
                } else if base_path.exists() && base_path.is_dir() {
                    Artifact::from_oci_dir(&base_path)?.get_manifest()?
                } else {
                    bail!("Artifact not found in local registry: {}", name)
                }
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
                // Check for both formats, prioritizing oci-archive
                let base_path = get_local_registry_path(&name);
                let archive_path = base_path.with_extension("ommx");

                if archive_path.exists() && archive_path.is_file() {
                    let mut artifact = Artifact::from_oci_archive(&archive_path)?;
                    artifact.push()?;
                } else if base_path.exists() && base_path.is_dir() {
                    let mut artifact = Artifact::from_oci_dir(&base_path)?;
                    artifact.push()?;
                } else {
                    bail!("Artifact not found in local registry: {}", name)
                }
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
            // Check for both formats, prioritizing oci-archive
            let base_path = get_local_registry_path(&name);
            let archive_path = base_path.with_extension("ommx");

            if archive_path.exists() && archive_path.is_file() {
                // Convert oci-archive to oci-archive (copy)
                std::fs::copy(&archive_path, output)?;
            } else if base_path.exists() && base_path.is_dir() {
                let mut artifact = Artifact::from_oci_dir(&base_path)?;
                artifact.save(output)?;
            } else {
                bail!("Artifact not found in local registry: {}", name)
            }
        }

        Command::Load { path } => {
            let mut artifact = Artifact::from_oci_archive(path)?;
            artifact.load()?;
        }

        Command::LocalRegistryPath { image_name } => {
            let name = ImageName::parse(image_name)?;
            let path = get_local_registry_path(&name);
            println!("{}", path.display());
        }

        Command::ImageDirectory { image_name } => {
            log::warn!(
                "The 'image-directory' command is deprecated since 2.1.0. \
                 Use 'local-registry-path' instead for dual format support."
            );
            let name = ImageName::parse(image_name)?;
            #[allow(deprecated)]
            let path = ommx::artifact::get_image_dir(&name);
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
