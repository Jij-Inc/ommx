//! Command-line interface shared by the standalone executable and Python SDK.
//!
//! Enable the `cli` feature to use [`run`]. Argument parsing, command dispatch,
//! and terminal output are owned by this module; Artifact and Local Registry
//! operations continue to use the SDK APIs that preserve their invariants.
//!
//! `ommx skill path` prints the absolute parent directory of bundled coding-agent
//! skills; `ommx skill path ommx` prints the OMMX skill directory itself.
//! `ommx plugin path` prints the plugin root, and `ommx plugin marketplace path`
//! prints its local marketplace root. The embedded bundle is materialized in a
//! content-addressed OS cache, or under `OMMX_PLUGIN_CACHE_DIR` when set.
//! No source checkout, network access,
//! or Artifact Local Registry is needed.
//!
//! # Compatibility
//!
//! The `cli` feature and [`run`] are part of the Rust SDK's stable API. Argument
//! parsing types and command handlers are private implementation details.
//!
//! Existing command syntax and documented behavior are preserved within a major
//! version. Help text, human-readable diagnostics, colors, and JSON whitespace or
//! object key order may change. Machine-readable JSON output is compatible at
//! the schema level.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::{ColoredString, Colorize};
use oci_spec::image::{Digest, ImageManifest};
use ommx::artifact::{
    fetch_remote_manifest, get_local_registry_root,
    local_registry::{
        AnonymousRefOptions, ArchiveInspectView, ArtifactListOptions, ArtifactRefRecord, GcBlob,
        GcDeleteReport, GcOptions, GcReport, LocalRegistry, OciDirRef, RefUpdate,
        RegistryListReport,
    },
    ImageRef, LocalArtifact,
};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

mod agent_plugin;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Show the version
    Version,

    /// Discover the bundled agent plugin
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },

    /// Discover bundled coding-agent skills
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
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

    /// List the images in the local registry
    List,

    /// Show the Manifest JSON, config, and unique layer sizes for local image refs
    Size {
        /// One or more container image names stored in the Local Registry
        #[clap(required = true)]
        image_names: Vec<String>,
    },

    /// Import an OCI archive or OCI Image Layout directory into the local registry
    Import {
        /// Path of OCI archive or OCI directory
        path: PathBuf,
    },

    /// Export an image in the local registry to an OCI archive
    Export {
        /// Container image name
        image_name: String,
        /// Output file name of OCI archive
        output: PathBuf,
    },

    /// Remove one image ref from the Local Registry.
    ///
    /// Content-addressed blobs are left in place for a later garbage collection.
    Rm {
        /// Container image name to remove.
        image_name: String,

        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,
    },

    /// Restore a removed Local Registry ref from its manifest digest.
    RestoreRef {
        /// Container image name to restore.
        image_name: String,

        /// Manifest digest printed by the remove or prune command.
        manifest_digest: Digest,

        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,
    },

    /// Import legacy path/tag OCI directories into the v3 local registry.
    ///
    /// Reformatting an Image Manifest as an Artifact Manifest is a separate explicit operation
    /// (`convert`, not yet exposed) that produces a new artifact under a new digest / new ref.
    ImportLegacy {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Replace existing v3 refs when a legacy entry has the same name but a different manifest.
        #[clap(long)]
        replace: bool,
    },

    /// Report or delete synthetic anonymous Local Registry refs.
    ///
    /// Manifest / blob CAS records are left in place; `gc` reclaims them.
    PruneAnonymous {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Explicit dry-run mode. This is the default unless --delete is passed.
        #[clap(long)]
        dry_run: bool,

        /// Delete anonymous refs instead of only reporting them.
        #[clap(long)]
        delete: bool,

        /// Include refs produced by anonymous Experiment sessions.
        #[clap(long)]
        experiments: bool,

        /// Include only refs at least this old. Accepts s, m, h, d suffixes.
        #[clap(long, value_parser = GcOptions::parse_grace_period)]
        older_than: Option<Duration>,

        /// Show manifest digest for each anonymous ref.
        #[clap(long)]
        show_digests: bool,
    },

    /// Report or delete Local Registry blobs unreachable from refs.
    ///
    /// All SQLite refs are GC roots, including Experiment checkpoint refs.
    /// Unreachable blobs newer than the grace period are deferred so active
    /// Run writes after the latest checkpoint are not removed.
    Gc {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Explicit dry-run mode. This is the default unless --delete is passed.
        #[clap(long)]
        dry_run: bool,

        /// Delete orphan candidates instead of only reporting them.
        #[clap(long)]
        delete: bool,

        /// Keep unreachable blobs newer than this duration. Accepts s, m, h, d suffixes.
        #[clap(long, default_value = "24h", value_parser = GcOptions::parse_grace_period)]
        grace_period: Duration,

        /// Show blob digests in GC detail output.
        #[clap(long)]
        show_digests: bool,
    },

    /// Deprecated alias for `import`.
    #[command(hide = true)]
    Load {
        /// Path of OCI archive or OCI directory
        path: PathBuf,
    },

    /// Deprecated alias for `export`.
    #[command(hide = true)]
    Save {
        /// Container image name
        image_name: String,
        /// Output file name of OCI archive
        output: PathBuf,
    },

    /// Manage Artifact v3 local registry
    #[command(hide = true)]
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommand,
    },
}

#[derive(Subcommand)]
enum PluginCommand {
    /// Print the absolute directory path of the bundled OMMX plugin
    Path {
        /// Select one plugin instead of the default
        #[clap(value_enum)]
        name: Option<PluginName>,
    },
    /// Discover the plugin marketplace that lists the bundled plugin
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
    },
}

#[derive(Clone, ValueEnum)]
enum PluginName {
    Ommx,
}

#[derive(Subcommand)]
enum MarketplaceCommand {
    /// Print the absolute directory path of the bundled plugin marketplace
    Path,
}

#[derive(Subcommand)]
enum SkillCommand {
    /// Print the absolute skills directory path for linking or copying into a project
    Path {
        /// Print one skill directory instead of the skills parent directory
        #[clap(value_enum)]
        name: Option<SkillName>,
    },
}

#[derive(Clone, ValueEnum)]
enum SkillName {
    Ommx,
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

    /// Report or delete synthetic anonymous Local Registry refs.
    ///
    /// `new_anonymous` writes artifacts under the synthetic ref
    /// `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>`
    /// so the SQLite Local Registry has a key to address the artifact
    /// under. This command reports or deletes every ref whose name + tag match
    /// that structure, including entries imported from registries with
    /// different `registry_id` prefixes. Manifest / blob CAS records
    /// are left in place; a future GC sweep will reclaim them.
    PruneAnonymous {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Explicit dry-run mode. This is the default unless --delete is passed.
        #[clap(long)]
        dry_run: bool,

        /// Delete anonymous refs instead of only reporting them.
        #[clap(long)]
        delete: bool,

        /// Include refs produced by anonymous Experiment sessions.
        #[clap(long)]
        experiments: bool,

        /// Include only refs at least this old. Accepts s, m, h, d suffixes.
        #[clap(long, value_parser = GcOptions::parse_grace_period)]
        older_than: Option<Duration>,

        /// Show manifest digest for each anonymous ref.
        #[clap(long)]
        show_digests: bool,
    },

    /// Report or delete Local Registry blobs unreachable from refs.
    ///
    /// All SQLite refs are GC roots, including Experiment checkpoint refs.
    /// Unreachable blobs newer than the grace period are deferred so active
    /// Run writes after the latest checkpoint are not removed.
    Gc {
        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Explicit dry-run mode. This is the default unless --delete is passed.
        #[clap(long)]
        dry_run: bool,

        /// Delete orphan candidates instead of only reporting them.
        #[clap(long)]
        delete: bool,

        /// Keep unreachable blobs newer than this duration. Accepts s, m, h, d suffixes.
        #[clap(long, default_value = "24h", value_parser = GcOptions::parse_grace_period)]
        grace_period: Duration,

        /// Show blob digests in GC detail output.
        #[clap(long)]
        show_digests: bool,
    },
}

enum ImageRefOrPath {
    Local(ImageRef),
    Remote(ImageRef),
    OciArchive(PathBuf),
    OciDir(PathBuf),
}

impl ImageRefOrPath {
    fn parse(input: &str) -> Result<Self> {
        let path: &Path = input.as_ref();
        if path.is_dir() {
            return Ok(Self::OciDir(path.to_path_buf()));
        }
        if path.is_file() {
            return Ok(Self::OciArchive(path.to_path_buf()));
        }
        if let Ok(name) = ImageRef::parse(input) {
            // SQLite Local Registry is the sole source for local
            // artifacts in v3. The pre-v3 path-tree layout under
            // `registry.root().join(image_name.as_path())` is no longer
            // auto-detected as "local"; users migrate it explicitly
            // via `ommx import-legacy`. After that, the ref resolves
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
            ImageRefOrPath::OciDir(path) => {
                let dir_ref = OciDirRef::read(path)?;
                let manifest_blob_path = path
                    .join("blobs")
                    .join(dir_ref.manifest_digest.algorithm().as_ref())
                    .join(dir_ref.manifest_digest.digest());
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
            // `Artifact.import_archive(file)` is the side-effecting
            // import path; `ommx inspect <archive>` should not mutate
            // the user's registry.
            ImageRefOrPath::OciArchive(path) => ArchiveInspectView::read(path)?.manifest,
            // `parse` only routes a ref to `Local` when SQLite resolves
            // it, so `LocalArtifact::open` should always succeed here;
            // if it doesn't, surface the SQLite-side migration message.
            ImageRefOrPath::Local(name) => LocalArtifact::open(name.clone())?
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
            ImageRefOrPath::Remote(name) => {
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
fn migration_hint_if_legacy_only(name: &ImageRef) -> Result<()> {
    if LocalRegistry::legacy_ref_path_in(get_local_registry_root(), name).exists() {
        bail!(
            "{name} exists only in the legacy local registry directory. \
             Run `ommx import-legacy` once to migrate it into the v3 \
             SQLite-backed registry, then retry."
        );
    }
    Ok(())
}

/// Fail with a "not in local registry" message, preferring the legacy
/// migration hint when applicable. Used by handlers (`Push`) where the
/// command has no remote fallback path and must terminate.
fn bail_not_found_locally(name: &ImageRef) -> Result<()> {
    migration_hint_if_legacy_only(name)?;
    bail!("Image not found in local: {}", name)
}

/// Run the OMMX CLI with arguments including the executable name.
///
/// Prints command output and help to stdout, and errors to stderr. Returns
/// `0` on success (including help/version), `2` for invalid arguments, or `1`
/// when command execution or writing terminal output fails.
///
/// This function does not exit the process or replace its global tracing
/// subscriber. A scoped terminal subscriber is used for this invocation.
///
/// ```no_run
/// let exit_code = ommx::cli::run(std::env::args_os());
/// ```
pub fn run<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = match Command::try_parse_from(args) {
        Ok(command) => command,
        Err(error) => {
            let exit_code = if error.use_stderr() { 2 } else { 0 };
            if let Err(error) = error.print() {
                let _ = writeln!(io::stderr(), "Error: {error}");
                return 1;
            }
            return exit_code;
        }
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();

    tracing::subscriber::with_default(subscriber, || match execute(command) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr(), "Error: {error:?}");
            1
        }
    })
}

fn execute(command: Command) -> Result<()> {
    match &command {
        Command::Plugin { command } => {
            let root = agent_plugin::path()?;
            let path = match command {
                PluginCommand::Path { .. } => root.join("plugins/ommx"),
                PluginCommand::Marketplace {
                    command: MarketplaceCommand::Path,
                } => root,
            };
            writeln!(io::stdout(), "{}", path.display())?;
        }
        Command::Skill {
            command: SkillCommand::Path { name },
        } => {
            let root = agent_plugin::path()?.join("plugins/ommx/skills");
            let path = if name.is_some() {
                root.join("ommx")
            } else {
                root
            };
            writeln!(io::stdout(), "{}", path.display())?;
        }
        Command::Version => {
            print_status("Version".blue().bold(), built_info::PKG_VERSION)?;
            print_status("Target".blue().bold(), built_info::TARGET)?;
            if let Some(hash) = built_info::GIT_COMMIT_HASH {
                print_status("Git Commit".blue().bold(), hash)?;
            }
        }
        Command::Inspect { image_name_or_path } => {
            let manifest = ImageRefOrPath::parse(image_name_or_path)?.get_manifest()?;
            // Manifest annotations are HashMaps. Sort JSON objects so output is
            // stable across processes and SDK dependency feature combinations.
            let mut manifest = serde_json::to_value(manifest)?;
            manifest.sort_all_objects();
            writeln!(io::stdout(), "{}", serde_json::to_string_pretty(&manifest)?)?;
        }

        Command::Push { image_name_or_path } => handle_push(image_name_or_path)?,

        Command::Pull { image_name } => handle_pull(image_name)?,

        Command::Import { path } => handle_import(path)?,

        Command::Export { image_name, output } => handle_export(image_name, output)?,

        Command::Rm { image_name, root } => handle_rm(image_name, root.as_ref())?,

        Command::RestoreRef {
            image_name,
            manifest_digest,
            root,
        } => handle_restore_ref(image_name, manifest_digest, root.as_ref())?,

        Command::List => {
            for image_name in ommx::artifact::get_images()? {
                writeln!(io::stdout(), "{image_name}")?;
            }
        }

        Command::Size { image_names } => handle_size(image_names)?,

        Command::ImportLegacy { root, replace } => handle_import_legacy(root.as_ref(), *replace)?,

        Command::PruneAnonymous {
            root,
            dry_run,
            delete,
            experiments,
            older_than,
            show_digests,
        } => handle_prune_anonymous(
            root.as_ref(),
            *dry_run,
            *delete,
            *experiments,
            *older_than,
            *show_digests,
        )?,

        Command::Gc {
            root,
            dry_run,
            delete,
            grace_period,
            show_digests,
        } => handle_gc(
            root.as_ref(),
            *dry_run,
            *delete,
            *grace_period,
            *show_digests,
        )?,

        Command::Load { path } => {
            writeln!(
                io::stderr(),
                "warning: `ommx load` is deprecated; use `ommx import` instead"
            )?;
            handle_import(path)?;
        }

        Command::Save { image_name, output } => {
            writeln!(
                io::stderr(),
                "warning: `ommx save` is deprecated; use `ommx export` instead"
            )?;
            handle_export(image_name, output)?;
        }

        Command::Artifact { command } => match command {
            ArtifactCommand::Import { root, replace } => {
                writeln!(
                    io::stderr(),
                    "warning: `ommx artifact import` is deprecated; \
                     use `ommx import-legacy` instead"
                )?;
                handle_import_legacy(root.as_ref(), *replace)?;
            }
            ArtifactCommand::PruneAnonymous {
                root,
                dry_run,
                delete,
                experiments,
                older_than,
                show_digests,
            } => {
                writeln!(
                    io::stderr(),
                    "warning: `ommx artifact prune-anonymous` is deprecated; \
                     use `ommx prune-anonymous` instead"
                )?;
                handle_prune_anonymous(
                    root.as_ref(),
                    *dry_run,
                    *delete,
                    *experiments,
                    *older_than,
                    *show_digests,
                )?;
            }
            ArtifactCommand::Gc {
                root,
                dry_run,
                delete,
                grace_period,
                show_digests,
            } => {
                writeln!(
                    io::stderr(),
                    "warning: `ommx artifact gc` is deprecated; use `ommx gc` instead"
                )?;
                handle_gc(
                    root.as_ref(),
                    *dry_run,
                    *delete,
                    *grace_period,
                    *show_digests,
                )?;
            }
        },
    }
    Ok(())
}

fn handle_push(image_name_or_path: &str) -> Result<()> {
    match ImageRefOrPath::parse(image_name_or_path)? {
        // v3 treats archive / OCI Image Layout dirs as exchange
        // formats; push always goes from the SQLite Local Registry.
        // Both paths bail with the same migration hint: import into
        // the registry first, then push by image name.
        ImageRefOrPath::OciDir(path) => bail!(
            "Cannot push OCI Image Layout directory `{}` directly. Run \
             `ommx import <dir>` to import it into the SQLite Local Registry, \
             then `ommx push <image_name>`.",
            path.display(),
        ),
        ImageRefOrPath::OciArchive(path) => bail!(
            "Cannot push OCI archive `{}` directly. Run `ommx import <file>` \
             to import it into the SQLite Local Registry, then \
             `ommx push <image_name>`. (Archive is an exchange format; v3 \
             pushes always source from the registry.)",
            path.display(),
        ),
        // CLI and Python `Artifact.push()` share the same native
        // code path: `LocalArtifact::push()`. `parse` only routes
        // SQLite-resident refs to `Local`, so `open` is the right
        // call (it returns the migration message on miss).
        ImageRefOrPath::Local(name) => {
            LocalArtifact::open(name)?.push()?;
        }
        ImageRefOrPath::Remote(name) => bail_not_found_locally(&name)?,
    }
    Ok(())
}

fn handle_pull(image_name: &str) -> Result<()> {
    // Route remote pull through `LocalRegistry::pull_image` so the
    // freshly pulled artifact lands in the v3 SQLite registry.
    let name = ImageRef::parse(image_name)?;
    let registry = std::sync::Arc::new(LocalRegistry::open_default()?);
    registry.pull_image(&name)?;
    Ok(())
}

fn handle_import(path: &Path) -> Result<()> {
    // Archives go through the native `import::archive` reader;
    // directories use `import::oci_dir`, which dispatches on Image /
    // Artifact Manifest. Using `fs::metadata` surfaces permission and
    // IO errors with the path attached, and rejects special files
    // before they reach the archive reader.
    let metadata =
        std::fs::metadata(path).with_context(|| format!("Failed to stat {}", path.display()))?;
    let registry = std::sync::Arc::new(LocalRegistry::open_default()?);
    if metadata.is_dir() {
        registry.import_oci_dir(path)?;
    } else if metadata.is_file() {
        registry.import_oci_archive(path)?;
    } else {
        bail!(
            "Path is neither a directory nor a regular file: {}",
            path.display()
        );
    }
    Ok(())
}

fn handle_export(image_name: &str, output: &Path) -> Result<()> {
    let name = ImageRef::parse(image_name)?;
    LocalArtifact::open(name)?.save(output)?;
    Ok(())
}

fn handle_size(image_names: &[String]) -> Result<()> {
    let registry = LocalRegistry::open_default()?;
    let sizes = image_names
        .iter()
        .map(|image_name| {
            let image_name = ImageRef::parse(image_name)?;
            let prefix = image_name.to_string();
            let report = registry.list_artifacts_with_options(
                Some(&prefix),
                &ArtifactListOptions {
                    include_internal: true,
                    strict: false,
                },
            )?;
            for warning in &report.warnings {
                tracing::warn!("{warning}");
            }
            let record = exact_artifact_record(report, &image_name)?;
            Ok((image_name, record.referenced_blob_size()?))
        })
        .collect::<Result<Vec<_>>>()?;

    for (image_name, size) in sizes {
        print_status("Image".blue().bold(), image_name)?;
        print_status(
            "Size".green().bold(),
            format_args!("{} ({size} bytes)", format_bytes(size)),
        )?;
    }
    Ok(())
}

fn exact_artifact_record(
    report: RegistryListReport<ArtifactRefRecord>,
    image_name: &ImageRef,
) -> Result<ArtifactRefRecord> {
    if let Some(record) = report
        .records
        .into_iter()
        .find(|record| record.image_name() == image_name)
    {
        return Ok(record);
    }
    if let Some(warning) = report
        .warnings
        .iter()
        .find(|warning| warning.image_name == image_name.to_string())
    {
        bail!("{warning}");
    }
    bail!("Artifact not found in the Local Registry: {image_name}")
}

fn handle_rm(image_name: &str, root: Option<&PathBuf>) -> Result<()> {
    let image_name = ImageRef::parse(image_name)?;
    let registry = open_registry(root)?;
    let Some(removed) = registry.remove_image_ref(&image_name)? else {
        print_status("Not Found".yellow().bold(), image_name)?;
        return Ok(());
    };
    print_status("Removed".red().bold(), &image_name)?;
    print_rollback(&image_name.to_string(), &removed.manifest_digest, root)?;
    print_status("Storage".blue().bold(), rm_storage_message())?;
    Ok(())
}

fn handle_restore_ref(
    image_name: &str,
    manifest_digest: &Digest,
    root: Option<&PathBuf>,
) -> Result<()> {
    let image_name = ImageRef::parse(image_name)?;
    let registry = open_registry(root)?;
    match registry.restore_image_ref(&image_name, manifest_digest)? {
        RefUpdate::Inserted => print_status("Restored".green().bold(), image_name)?,
        RefUpdate::Unchanged => print_status("Unchanged".blue().bold(), image_name)?,
        RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } => bail!(
            "Cannot restore {image_name} to {incoming_manifest_digest}: ref currently points to \
             {existing_manifest_digest}"
        ),
        RefUpdate::Replaced { .. } => {
            unreachable!("restore_image_ref never replaces an existing ref")
        }
    }
    Ok(())
}

fn open_registry(root: Option<&PathBuf>) -> Result<LocalRegistry> {
    if let Some(root) = root {
        LocalRegistry::open(root)
    } else {
        LocalRegistry::open_default()
    }
}

fn handle_import_legacy(root: Option<&PathBuf>, replace: bool) -> Result<()> {
    let registry = open_registry(root)?;
    let report = if replace {
        registry.replace_legacy_layout()?
    } else {
        registry.import_legacy_layout()?
    };
    print_status(
        "Imported".green().bold(),
        format_args!(
            "{} legacy OCI dir(s) into {}",
            report.imported_dirs,
            registry.root().display()
        ),
    )?;
    print_status(
        "Scanned".blue().bold(),
        format_args!("{} legacy OCI dir(s)", report.scanned_dirs),
    )?;
    print_status(
        "Verified".blue().bold(),
        format_args!("{} existing ref(s)", report.verified_dirs),
    )?;
    print_status(
        "Replaced".yellow().bold(),
        format_args!("{} existing ref(s)", report.replaced_refs),
    )?;
    if report.conflicted_dirs > 0 {
        print_status(
            "Skipped".yellow().bold(),
            format_args!(
                "{} conflicting ref(s); rerun with --replace to overwrite them",
                report.conflicted_dirs
            ),
        )?;
    }
    Ok(())
}

fn handle_prune_anonymous(
    root: Option<&PathBuf>,
    dry_run: bool,
    delete: bool,
    experiments: bool,
    older_than: Option<Duration>,
    show_digests: bool,
) -> Result<()> {
    if dry_run && delete {
        bail!("--dry-run and --delete cannot be used together");
    }
    let registry = open_registry(root)?;
    let options = AnonymousRefOptions {
        include_experiments: experiments,
        older_than,
    };
    let to_remove = registry.list_anonymous_refs(&options)?;
    if to_remove.is_empty() {
        print_status("Clean".green().bold(), "no matching anonymous refs found")?;
    } else if delete {
        let removed = registry.prune_anonymous_refs(&options)?;
        print_status(
            "Removed".red().bold(),
            format_args!("{} anonymous ref(s)", removed.len()),
        )?;
        for r in &removed {
            print_anonymous_ref(&r.name, &r.reference, &r.manifest_digest, show_digests)?;
            print_rollback(
                &format!("{}:{}", r.name, r.reference),
                &r.manifest_digest,
                root,
            )?;
        }
    } else {
        print_status(
            "Candidates".yellow().bold(),
            format_args!("{} anonymous ref(s)", to_remove.len()),
        )?;
        for r in &to_remove {
            print_anonymous_ref(&r.name, &r.reference, &r.manifest_digest, show_digests)?;
        }
        print_status(
            "Dry Run".yellow().bold(),
            "registry unchanged; pass --delete to apply",
        )?;
    }
    Ok(())
}

fn handle_gc(
    root: Option<&PathBuf>,
    dry_run: bool,
    delete: bool,
    grace_period: Duration,
    show_digests: bool,
) -> Result<()> {
    if dry_run && delete {
        bail!("--dry-run and --delete cannot be used together");
    }
    let registry = open_registry(root)?;
    let options = GcOptions {
        grace_period,
        ..GcOptions::default()
    };
    if delete {
        let result = registry.gc(&options)?;
        print_gc_delete_report(&registry, &result, show_digests)?;
    } else {
        let report = registry.gc_report(&options)?;
        print_gc_report(&registry, &report, show_digests)?;
        print_status(
            "Dry Run".yellow().bold(),
            "registry unchanged; pass --delete to apply",
        )?;
    }
    Ok(())
}

fn print_gc_delete_report(
    registry: &LocalRegistry,
    result: &GcDeleteReport,
    show_digests: bool,
) -> Result<()> {
    print_gc_report(registry, &result.report, show_digests)?;
    print_status(
        "Deleted".red().bold(),
        format_args!(
            "{} orphan blob(s), {}",
            result.deleted_blobs.len(),
            format_bytes(result.deleted_size())
        ),
    )?;
    if show_digests {
        print_blob_list(&result.deleted_blobs)?;
    }
    if !result.skipped_blobs.is_empty() {
        print_status(
            "Skipped".yellow().bold(),
            format_args!(
                "{} blob(s) changed before deletion",
                result.skipped_blobs.len()
            ),
        )?;
        if show_digests {
            print_blob_list(&result.skipped_blobs)?;
        }
    }
    Ok(())
}

fn print_gc_report(registry: &LocalRegistry, report: &GcReport, show_digests: bool) -> Result<()> {
    print_status("Registry".blue().bold(), registry.root().display())?;
    print_status(
        "Roots".blue().bold(),
        format_args!("{} ref/protected digest(s)", report.roots.len()),
    )?;
    print_status(
        "Reachable".green().bold(),
        format_args!(
            "{} blob(s), {}",
            report.reachable_blobs.len(),
            format_bytes(report.reachable_size())
        ),
    )?;
    print_status(
        "Orphans".yellow().bold(),
        format_args!(
            "{} candidate blob(s), {}",
            report.orphan_candidates.len(),
            format_bytes(report.orphan_candidate_size())
        ),
    )?;
    if show_digests {
        print_blob_list(&report.orphan_candidates)?;
    }
    print_status(
        "Deferred".yellow().bold(),
        format_args!(
            "{} blob(s), {}",
            report.deferred_blobs.len(),
            format_bytes(report.deferred_size())
        ),
    )?;
    if show_digests {
        print_blob_list(&report.deferred_blobs)?;
    }
    if !report.missing_blobs.is_empty() {
        print_status(
            "Missing".red().bold(),
            format_args!("{} referenced blob(s)", report.missing_blobs.len()),
        )?;
        if show_digests {
            for missing in &report.missing_blobs {
                writeln!(
                    io::stdout(),
                    "  {}  {:?}",
                    missing.digest.to_string().dimmed(),
                    missing.kind
                )?;
            }
        }
    }
    if !report.invalid_manifests.is_empty() {
        print_status(
            "Invalid".red().bold(),
            format_args!("{} manifest blob(s)", report.invalid_manifests.len()),
        )?;
        if show_digests {
            for invalid in &report.invalid_manifests {
                writeln!(
                    io::stdout(),
                    "  {}  {:?}: {}",
                    invalid.digest.to_string().dimmed(),
                    invalid.kind,
                    invalid.error
                )?;
            }
        }
    }
    Ok(())
}

fn print_status(label: ColoredString, message: impl std::fmt::Display) -> Result<()> {
    writeln!(io::stdout(), "{label:>12} {message}")?;
    Ok(())
}

fn print_anonymous_ref(
    name: &str,
    reference: &str,
    digest: impl std::fmt::Display,
    show_digests: bool,
) -> Result<()> {
    if show_digests {
        writeln!(
            io::stdout(),
            "  {}:{}  {}  {}",
            name.dimmed(),
            reference,
            "->".dimmed(),
            digest
        )?;
    } else {
        writeln!(io::stdout(), "  {}:{}", name.dimmed(), reference)?;
    }
    Ok(())
}

fn print_rollback(
    image_name: &str,
    manifest_digest: &Digest,
    root: Option<&PathBuf>,
) -> Result<()> {
    print_status(
        "Rollback".blue().bold(),
        rollback_command(image_name, manifest_digest, root),
    )
}

fn rm_storage_message() -> &'static str {
    "Unreferenced data remains until a later `ommx gc --delete` removes it after the grace period."
}

fn rollback_command(image_name: &str, manifest_digest: &Digest, root: Option<&PathBuf>) -> String {
    let mut command = format!(
        "ommx restore-ref {} {}",
        shell_quote(image_name),
        shell_quote(manifest_digest.as_ref())
    );
    if let Some(root) = root {
        command.push_str(" --root ");
        command.push_str(&shell_quote(&root.display().to_string()));
    }
    command
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn print_blob_list(blobs: &[GcBlob]) -> Result<()> {
    for blob in blobs {
        writeln!(
            io::stdout(),
            "  {}  {}",
            blob.digest.to_string().dimmed(),
            format_bytes(blob.size)
        )?;
    }
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ommx::artifact::local_registry::{RegistryListWarning, RegistryListWarningStage};

    #[test]
    fn run_returns_after_help_errors_and_repeated_execution() {
        assert_eq!(run(["ommx", "--help"]), 0);
        assert_eq!(run(["ommx", "inspect"]), 2);
        assert_eq!(run(["ommx", "version"]), 0);
        assert_eq!(run(["ommx", "version"]), 0);
    }

    #[test]
    fn skill_path_parses_parent_and_named_paths() {
        for (args, expected) in [
            (vec!["ommx", "skill", "path"], None),
            (vec!["ommx", "skill", "path", "ommx"], Some("ommx")),
        ] {
            let Command::Skill {
                command: SkillCommand::Path { name },
            } = Command::try_parse_from(args).unwrap()
            else {
                panic!("expected skill path command");
            };
            assert_eq!(name.map(|SkillName::Ommx| "ommx"), expected);
        }
        assert!(Command::try_parse_from(["ommx", "skill"]).is_err());
        assert!(Command::try_parse_from(["ommx", "skill", "path", "../other"]).is_err());
        assert!(Command::try_parse_from(["ommx", "skill", "path", "--name", "ommx"]).is_err());
    }

    #[test]
    fn plugin_paths_follow_jijmodeling_command_syntax() {
        for args in [
            vec!["ommx", "plugin", "path"],
            vec!["ommx", "plugin", "path", "ommx"],
        ] {
            assert!(matches!(
                Command::try_parse_from(args).unwrap(),
                Command::Plugin {
                    command: PluginCommand::Path { .. }
                }
            ));
        }
        assert!(matches!(
            Command::try_parse_from(["ommx", "plugin", "marketplace", "path"]).unwrap(),
            Command::Plugin {
                command: PluginCommand::Marketplace {
                    command: MarketplaceCommand::Path
                }
            }
        ));
        for args in [
            vec!["ommx", "plugin"],
            vec!["ommx", "plugin", "marketplace"],
            vec!["ommx", "plugin", "path", "other"],
            vec!["ommx", "plugin", "path", "--name", "ommx"],
        ] {
            assert!(Command::try_parse_from(args).is_err());
        }
    }

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn restore_ref_cli_parses_rollback_command() {
        let command = Command::try_parse_from([
            "ommx",
            "restore-ref",
            "example.com/ommx/demo:deleted",
            DIGEST,
            "--root",
            "/tmp/registry",
        ])
        .unwrap();
        let Command::RestoreRef {
            image_name,
            manifest_digest,
            root,
        } = command
        else {
            panic!("expected restore-ref command");
        };
        assert_eq!(image_name, "example.com/ommx/demo:deleted");
        assert_eq!(manifest_digest.as_ref(), DIGEST);
        assert_eq!(root, Some(PathBuf::from("/tmp/registry")));
    }

    #[test]
    fn rollback_command_is_shell_safe_and_preserves_root() {
        let digest = DIGEST.parse().unwrap();
        assert_eq!(
            rollback_command(
                "example.com/ommx/demo:deleted",
                &digest,
                Some(&PathBuf::from("/tmp/registry with ' quote")),
            ),
            concat!(
                "ommx restore-ref 'example.com/ommx/demo:deleted' '",
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef' ",
                "--root '/tmp/registry with '\"'\"' quote'"
            )
        );
    }

    #[test]
    fn rm_has_no_implicit_gc_option_and_explains_storage_lifecycle() {
        let command = Command::try_parse_from([
            "ommx",
            "rm",
            "example.com/ommx/demo:deleted",
            "--root",
            "/tmp/registry",
        ])
        .unwrap();
        let Command::Rm { image_name, root } = command else {
            panic!("expected rm command");
        };
        assert_eq!(image_name, "example.com/ommx/demo:deleted");
        assert_eq!(root, Some(PathBuf::from("/tmp/registry")));
        assert!(
            Command::try_parse_from(["ommx", "rm", "example.com/ommx/demo:deleted", "--gc",])
                .is_err()
        );
        assert_eq!(
            rm_storage_message(),
            "Unreferenced data remains until a later `ommx gc --delete` removes it after the grace period."
        );
    }

    #[test]
    fn size_cli_accepts_multiple_images() {
        let command = Command::try_parse_from([
            "ommx",
            "size",
            "example.com/ommx/experiment:first",
            "example.com/ommx/experiment:second",
        ])
        .unwrap();
        let Command::Size { image_names } = command else {
            panic!("expected size command");
        };
        assert_eq!(
            image_names,
            [
                "example.com/ommx/experiment:first",
                "example.com/ommx/experiment:second"
            ]
        );
    }

    #[test]
    fn size_cli_requires_at_least_one_image() {
        assert!(Command::try_parse_from(["ommx", "size"]).is_err());
    }

    #[test]
    fn size_cli_uses_the_default_registry() {
        assert!(Command::try_parse_from([
            "ommx",
            "size",
            "example.com/ommx/experiment:latest",
            "--root",
            "/tmp/registry",
        ])
        .is_err());
    }

    #[test]
    fn size_cli_help_names_the_counted_manifest_parts() {
        let help = match Command::try_parse_from(["ommx", "size", "--help"]) {
            Ok(_) => panic!("--help must stop argument parsing"),
            Err(error) => error.to_string(),
        };
        assert!(help.contains("Manifest JSON, config, and unique layer sizes"));
        assert!(!help.contains("reachable"));
    }

    #[test]
    fn size_reports_target_corruption_instead_of_not_found() {
        let image_name = ImageRef::parse("example.com/ommx/experiment:corrupt").unwrap();
        let error = exact_artifact_record(
            RegistryListReport {
                records: Vec::new(),
                warnings: vec![RegistryListWarning {
                    image_name: image_name.to_string(),
                    manifest_digest: DIGEST.to_string(),
                    stage: RegistryListWarningStage::ManifestCacheRepair,
                    message: "Invalid cached Manifest; CAS repair failed".to_string(),
                }],
            },
            &image_name,
        )
        .expect_err("target corruption must be returned as an error");
        let message = format!("{error:#}");
        assert!(message.contains("CAS repair failed"));
        assert!(!message.contains("Artifact not found"));
    }
}
