use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use ocipkg::{oci_spec::image::ImageManifest, ImageName};
use ommx::artifact::{
    get_image_dir,
    local_registry::{
        import_oci_archive, import_oci_dir, pull_image, LocalRegistry, RefConflictPolicy,
    },
    Artifact, LocalArtifact,
};
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

    /// Manage Artifact v3 local registry
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommand,
    },
}

#[derive(Subcommand)]
enum ArtifactCommand {
    /// Import legacy path/tag OCI directories into the v3 local registry, preserving manifest digest.
    ///
    /// Reformatting an Image Manifest as an Artifact Manifest is a separate explicit operation
    /// (`convert`, not yet exposed) that produces a new artifact under a new digest / new ref.
    Import {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Replace existing v3 refs when a legacy entry has the same name but a different manifest.
        #[clap(long)]
        replace: bool,
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
            // Prefer the SQLite Local Registry: anything imported via
            // `ommx load` or `ommx pull` (post-v3) lands there and the
            // legacy disk OCI dir is only kept around as the
            // lazy-auto-migration cache. Falling back to the legacy
            // dir keeps pre-v3 user state addressable until Step C
            // removes that path entirely.
            if LocalArtifact::try_open(name.clone())?.is_some() {
                return Ok(Self::Local(name));
            }
            if get_image_dir(&name).exists() {
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
                let image_dir = get_image_dir(name);
                Artifact::from_oci_dir(&image_dir)?.get_manifest()?
            }
            ImageNameOrPath::Remote(name) => Artifact::from_remote(name.clone())?.get_manifest()?,
        };
        Ok(manifest)
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
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
            // The CLI and the Python `Artifact.push()` share the same
            // native code path: `LocalArtifact::push()`. `try_open`
            // resolves via the SQLite Local Registry (post-v3 default);
            // when only a legacy disk OCI dir is present `open` would
            // bail with the "run `ommx artifact import`" message, but
            // the `parse` dispatch above already routed that case to
            // `Local(name)`, so fall back to the legacy push for now.
            // Step C removes the legacy branch.
            ImageNameOrPath::Local(name) => {
                if let Some(artifact) = LocalArtifact::try_open(name.clone())? {
                    artifact.push()?;
                } else {
                    let image_dir = get_image_dir(&name);
                    let mut artifact = Artifact::from_oci_dir(&image_dir)?;
                    artifact.push()?;
                }
            }
            ImageNameOrPath::Remote(name) => {
                bail!("Image not found in local: {}", name)
            }
        },

        Command::Pull { image_name } => {
            // Route remote pull through `local_registry::pull_image` so the
            // freshly pulled artifact lands in the v3 SQLite registry. The
            // legacy OCI dir is still produced as the ocipkg-based stage 1
            // (see `import::remote::pull_image`); a follow-up PR replaces
            // that stage with a native streaming pull and the call site
            // here stays unchanged.
            let name = ImageName::parse(image_name)?;
            let registry = std::sync::Arc::new(LocalRegistry::open_default()?);
            pull_image(&registry, &name)?;
        }

        Command::Save { image_name, output } => {
            let name = ImageName::parse(image_name)?;
            let image_dir = get_image_dir(&name);
            let mut artifact = Artifact::from_oci_dir(&image_dir)?;
            artifact.save(output)?;
        }

        Command::Load { path } => {
            // The CLI flag advertises "OCI archive or OCI directory", so
            // dispatch on what the path actually is. Archives go through
            // the ocipkg-based stage-1 pipeline in `import::archive`;
            // directories use the native `import::oci_dir` path that
            // dispatches on Image / Artifact Manifest. Using
            // `fs::metadata` (rather than `Path::exists()` /
            // `Path::is_dir()`) surfaces permission and IO errors with
            // the path attached, and rejects special files (FIFO,
            // socket, device) explicitly instead of sending them to the
            // archive branch where they would fail with an opaque
            // ocipkg / tar error.
            let metadata = std::fs::metadata(path)
                .with_context(|| format!("Failed to stat {}", path.display()))?;
            let registry = std::sync::Arc::new(LocalRegistry::open_default()?);
            if metadata.is_dir() {
                import_oci_dir(registry.index(), registry.blobs(), path)?;
            } else if metadata.is_file() {
                import_oci_archive(&registry, path)?;
            } else {
                bail!(
                    "Path is neither a directory nor a regular file: {}",
                    path.display()
                );
            }
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

        Command::Artifact { command } => match command {
            ArtifactCommand::Import { root, replace } => {
                let registry = if let Some(root) = root {
                    LocalRegistry::open(root)?
                } else {
                    LocalRegistry::open_default()?
                };
                let policy = if *replace {
                    RefConflictPolicy::Replace
                } else {
                    RefConflictPolicy::KeepExisting
                };
                let report = registry.import_legacy_layout_with_policy(policy)?;
                println!(
                    "Imported {} legacy OCI dir(s) into {}",
                    report.imported_dirs,
                    registry.root().display()
                );
                println!("Scanned {} legacy OCI dir(s)", report.scanned_dirs);
                println!("Verified {} existing ref(s)", report.verified_dirs);
                println!("Replaced {} existing ref(s)", report.replaced_refs);
                if report.conflicted_dirs > 0 {
                    println!(
                        "Skipped {} conflicting ref(s); rerun with --replace to overwrite them",
                        report.conflicted_dirs
                    );
                }
            }
        },
    }
    Ok(())
}
