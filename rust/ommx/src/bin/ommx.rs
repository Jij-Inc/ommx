use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use ocipkg::{oci_spec::image::ImageManifest, ImageName};
use ommx::artifact::{
    fetch_remote_manifest, get_image_dir,
    local_registry::{
        import_oci_archive, import_oci_dir, oci_dir_ref, pull_image, read_archive_manifest,
        LocalRegistry, RefConflictPolicy,
    },
    LocalArtifact,
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

    /// Delete every SQLite ref produced by `ArtifactBuilder.new_anonymous`.
    ///
    /// `new_anonymous` writes artifacts under the synthetic ref
    /// `<registry-id8>.ommx.local/anonymous:<local-timestamp>` so the
    /// SQLite Local Registry has a key to address the artifact under.
    /// This command deletes every ref whose name ends with
    /// `.ommx.local/anonymous`, including entries imported from
    /// registries with different `registry_id` prefixes. Manifest /
    /// blob CAS records are left in place; a future GC sweep will
    /// reclaim them.
    PruneAnonymous {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// List refs that would be removed without modifying the registry.
        #[clap(long)]
        dry_run: bool,
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
            // SQLite Local Registry is the sole source for local
            // artifacts in v3. The pre-v3 path-tree layout under
            // `registry.root().join(image_name.as_path())` is no longer
            // auto-detected as "local"; users migrate it explicitly
            // via `ommx artifact import`. After that, the ref resolves
            // through SQLite like any other v3 artifact.
            //
            // The SQLite probe is best-effort: an unopenable registry
            // (corrupt DB, read-only filesystem, permission denied) is
            // *not* fatal for a remote-targeted command like
            // `ommx push <ghcr ref>` or `ommx inspect <remote>`. We log
            // the failure and fall through to the remote branch.
            match LocalArtifact::try_open(name.clone()) {
                Ok(Some(_)) => return Ok(Self::Local(name)),
                Ok(None) => {}
                Err(e) => {
                    tracing::debug!(
                        "SQLite Local Registry probe for {name} failed ({e:#}); \
                         treating ref as not-local-in-SQLite"
                    );
                }
            }
            return Ok(Self::Remote(name));
        }
        bail!("Invalid input: {}", input)
    }

    fn get_manifest(&self) -> Result<ImageManifest> {
        let manifest = match self {
            // OCI Image Layout directory inspect: read the manifest
            // descriptor's digest out of `index.json` (via the existing
            // `oci_dir_ref`) and load the manifest blob directly from
            // disk. Avoids importing into SQLite for a read-only op.
            ImageNameOrPath::OciDir(path) => {
                let dir_ref = oci_dir_ref(path)?;
                let manifest_blob_path = path
                    .join("blobs")
                    .join("sha256")
                    .join(dir_ref.manifest_digest.trim_start_matches("sha256:"));
                let bytes = std::fs::read(&manifest_blob_path).with_context(|| {
                    format!(
                        "Failed to read manifest blob at {}",
                        manifest_blob_path.display()
                    )
                })?;
                serde_json::from_slice::<ImageManifest>(&bytes).with_context(|| {
                    format!(
                        "Failed to parse OCI image manifest at {}",
                        manifest_blob_path.display()
                    )
                })?
            }
            // Read-only inspect: a native tar pre-scan extracts the
            // manifest blob without touching the SQLite Local Registry.
            // `Artifact.load_archive(file)` is the side-effecting
            // import path; `ommx inspect <archive>` should not mutate
            // the user's registry.
            ImageNameOrPath::OciArchive(path) => read_archive_manifest(path)?,
            // `parse` only routes a ref to `Local` when SQLite resolves
            // it, so `LocalArtifact::open` should always succeed here;
            // if it doesn't, surface the SQLite-side migration message.
            ImageNameOrPath::Local(name) => LocalArtifact::open(name.clone())?
                .get_manifest()?
                .clone()
                .into_inner(),
            // `Remote` here also covers pre-v3 users whose artifact is
            // only in the legacy disk dir (SQLite misses → parse falls
            // through to `Remote`). Bail with the migration hint before
            // initiating a network fetch so `ommx inspect` does not
            // silently look up a ref the user already has locally.
            // Manifest-only fetch (no blob pull, no SQLite write) keeps
            // inspect cheap; users who want the bytes locally run
            // `ommx pull <name>`.
            ImageNameOrPath::Remote(name) => {
                migration_hint_if_legacy_only(name)?;
                fetch_remote_manifest(name)?
            }
        };
        Ok(manifest)
    }
}

/// Bail with the pre-v3 → v3 migration hint when a legacy v2-shaped
/// OCI directory exists at the user's local registry root for this
/// image. Used by handlers (`Inspect`, `Save`) where the next step
/// would otherwise contact the network for what is in fact a local
/// pre-v3 artifact. Returns `Ok(())` when no legacy dir is present,
/// letting callers proceed with their normal remote / local fallback.
fn migration_hint_if_legacy_only(name: &ImageName) -> Result<()> {
    if get_image_dir(name).exists() {
        bail!(
            "{name} exists only in the legacy local registry directory. \
             Run `ommx artifact import` once to migrate it into the v3 \
             SQLite-backed registry, then retry."
        );
    }
    Ok(())
}

/// Fail with a "not in local registry" message, preferring the legacy
/// migration hint when applicable. Used by handlers (`Push`) where the
/// command has no remote fallback path and must terminate.
fn bail_not_found_locally(name: &ImageName) -> Result<()> {
    migration_hint_if_legacy_only(name)?;
    bail!("Image not found in local: {}", name)
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
        Command::Inspect { image_name_or_path } => {
            let manifest = ImageNameOrPath::parse(image_name_or_path)?.get_manifest()?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }

        Command::Push { image_name_or_path } => match ImageNameOrPath::parse(image_name_or_path)? {
            // v3 treats archive / OCI Image Layout dirs as exchange
            // formats; push always goes from the SQLite Local Registry.
            // Both paths bail with the same migration hint: load into
            // the registry first, then push by image name.
            ImageNameOrPath::OciDir(path) => bail!(
                "Cannot push OCI Image Layout directory `{}` directly. Run \
                 `ommx load <dir>` to import it into the SQLite Local Registry, \
                 then `ommx push <image_name>`.",
                path.display(),
            ),
            ImageNameOrPath::OciArchive(path) => bail!(
                "Cannot push OCI archive `{}` directly. Run `ommx load <file>` \
                 to import it into the SQLite Local Registry, then \
                 `ommx push <image_name>`. (Archive is an exchange format; v3 \
                 pushes always source from the registry.)",
                path.display(),
            ),
            // CLI and Python `Artifact.push()` share the same native
            // code path: `LocalArtifact::push()`. `parse` only routes
            // SQLite-resident refs to `Local`, so `open` is the right
            // call (it returns the migration message on miss).
            ImageNameOrPath::Local(name) => {
                LocalArtifact::open(name)?.push()?;
            }
            ImageNameOrPath::Remote(name) => bail_not_found_locally(&name)?,
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
            LocalArtifact::open(name)?.save(output)?;
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
            ArtifactCommand::PruneAnonymous { root, dry_run } => {
                let registry = if let Some(root) = root {
                    LocalRegistry::open(root)?
                } else {
                    LocalRegistry::open_default()?
                };
                let to_remove = registry.list_anonymous_artifact_refs()?;
                if to_remove.is_empty() {
                    println!("No anonymous artifact refs found.");
                } else if *dry_run {
                    println!(
                        "Would remove {} anonymous artifact ref(s):",
                        to_remove.len()
                    );
                    for r in &to_remove {
                        println!("  {}:{}  →  {}", r.name, r.reference, r.manifest_digest);
                    }
                    println!("(--dry-run: registry unchanged)");
                } else {
                    let removed = registry.prune_anonymous_artifact_refs()?;
                    println!("Removed {} anonymous artifact ref(s):", removed.len());
                    for r in &removed {
                        println!("  {}:{}", r.name, r.reference);
                    }
                }
            }
        },
    }
    Ok(())
}
