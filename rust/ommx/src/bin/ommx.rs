use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::{ColoredString, Colorize};
use oci_spec::image::ImageManifest;
use ommx::artifact::{
    fetch_remote_manifest, get_local_registry_root,
    local_registry::{
        AnonymousRefOptions, ArchiveInspectView, GcBlob, GcDeleteReport, GcOptions, GcReport,
        LocalRegistry, OciDirRef,
    },
    ImageRef, LocalArtifact,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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

    /// List the images in the local registry
    List,

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
    /// Content-addressed blobs are left in place unless `--gc` is passed.
    Rm {
        /// Container image name to remove.
        image_name: String,

        /// Local registry root. Defaults to OMMX_LOCAL_REGISTRY_ROOT or the OS default data dir.
        #[clap(long)]
        root: Option<PathBuf>,

        /// Run Local Registry garbage collection after removing the ref.
        #[clap(long)]
        gc: bool,

        /// Keep unreachable blobs newer than this duration during `--gc`.
        #[clap(
            long,
            default_value = "24h",
            value_parser = GcOptions::parse_grace_period
        )]
        gc_grace_period: Duration,
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
            print_status("Version".blue().bold(), built_info::PKG_VERSION);
            print_status("Target".blue().bold(), built_info::TARGET);
            if let Some(hash) = built_info::GIT_COMMIT_HASH {
                print_status("Git Commit".blue().bold(), hash);
            }
        }
        Command::Inspect { image_name_or_path } => {
            let manifest = ImageRefOrPath::parse(image_name_or_path)?.get_manifest()?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }

        Command::Push { image_name_or_path } => handle_push(image_name_or_path)?,

        Command::Pull { image_name } => handle_pull(image_name)?,

        Command::Import { path } => handle_import(path)?,

        Command::Export { image_name, output } => handle_export(image_name, output)?,

        Command::Rm {
            image_name,
            root,
            gc,
            gc_grace_period,
        } => handle_rm(image_name, root.as_ref(), *gc, *gc_grace_period)?,

        Command::List => {
            for image_name in ommx::artifact::get_images()? {
                println!("{image_name}");
            }
        }

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
            eprintln!("warning: `ommx load` is deprecated; use `ommx import` instead");
            handle_import(path)?;
        }

        Command::Save { image_name, output } => {
            eprintln!("warning: `ommx save` is deprecated; use `ommx export` instead");
            handle_export(image_name, output)?;
        }

        Command::Artifact { command } => match command {
            ArtifactCommand::Import { root, replace } => {
                eprintln!(
                    "warning: `ommx artifact import` is deprecated; \
                     use `ommx import-legacy` instead"
                );
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
                eprintln!(
                    "warning: `ommx artifact prune-anonymous` is deprecated; \
                     use `ommx prune-anonymous` instead"
                );
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
                eprintln!("warning: `ommx artifact gc` is deprecated; use `ommx gc` instead");
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

fn handle_rm(
    image_name: &str,
    root: Option<&PathBuf>,
    run_gc: bool,
    gc_grace_period: Duration,
) -> Result<()> {
    let image_name = ImageRef::parse(image_name)?;
    let registry = open_registry(root)?;
    if registry.remove_image_ref(&image_name)? {
        print_status("Removed".red().bold(), image_name);
    } else {
        print_status("Not Found".yellow().bold(), image_name);
        return Ok(());
    }
    if run_gc {
        let result = registry.gc(&GcOptions {
            grace_period: gc_grace_period,
            ..GcOptions::default()
        })?;
        print_gc_delete_report(&registry, &result, false);
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
    );
    print_status(
        "Scanned".blue().bold(),
        format_args!("{} legacy OCI dir(s)", report.scanned_dirs),
    );
    print_status(
        "Verified".blue().bold(),
        format_args!("{} existing ref(s)", report.verified_dirs),
    );
    print_status(
        "Replaced".yellow().bold(),
        format_args!("{} existing ref(s)", report.replaced_refs),
    );
    if report.conflicted_dirs > 0 {
        print_status(
            "Skipped".yellow().bold(),
            format_args!(
                "{} conflicting ref(s); rerun with --replace to overwrite them",
                report.conflicted_dirs
            ),
        );
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
        print_status("Clean".green().bold(), "no matching anonymous refs found");
    } else if delete {
        let removed = registry.prune_anonymous_refs(&options)?;
        print_status(
            "Removed".red().bold(),
            format_args!("{} anonymous ref(s)", removed.len()),
        );
        for r in &removed {
            print_anonymous_ref(&r.name, &r.reference, &r.manifest_digest, show_digests);
        }
    } else {
        print_status(
            "Candidates".yellow().bold(),
            format_args!("{} anonymous ref(s)", to_remove.len()),
        );
        for r in &to_remove {
            print_anonymous_ref(&r.name, &r.reference, &r.manifest_digest, show_digests);
        }
        print_status(
            "Dry Run".yellow().bold(),
            "registry unchanged; pass --delete to apply",
        );
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
        print_gc_delete_report(&registry, &result, show_digests);
    } else {
        let report = registry.gc_report(&options)?;
        print_gc_report(&registry, &report, show_digests);
        print_status(
            "Dry Run".yellow().bold(),
            "registry unchanged; pass --delete to apply",
        );
    }
    Ok(())
}

fn print_gc_delete_report(registry: &LocalRegistry, result: &GcDeleteReport, show_digests: bool) {
    print_gc_report(registry, &result.report, show_digests);
    print_status(
        "Deleted".red().bold(),
        format_args!(
            "{} orphan blob(s), {}",
            result.deleted_blobs.len(),
            format_bytes(result.deleted_size())
        ),
    );
    if show_digests {
        print_blob_list(&result.deleted_blobs);
    }
    if !result.skipped_blobs.is_empty() {
        print_status(
            "Skipped".yellow().bold(),
            format_args!(
                "{} blob(s) changed before deletion",
                result.skipped_blobs.len()
            ),
        );
        if show_digests {
            print_blob_list(&result.skipped_blobs);
        }
    }
}

fn print_gc_report(registry: &LocalRegistry, report: &GcReport, show_digests: bool) {
    print_status("Registry".blue().bold(), registry.root().display());
    print_status(
        "Roots".blue().bold(),
        format_args!("{} ref/protected digest(s)", report.roots.len()),
    );
    print_status(
        "Reachable".green().bold(),
        format_args!(
            "{} blob(s), {}",
            report.reachable_blobs.len(),
            format_bytes(report.reachable_size())
        ),
    );
    print_status(
        "Orphans".yellow().bold(),
        format_args!(
            "{} candidate blob(s), {}",
            report.orphan_candidates.len(),
            format_bytes(report.orphan_candidate_size())
        ),
    );
    if show_digests {
        print_blob_list(&report.orphan_candidates);
    }
    print_status(
        "Deferred".yellow().bold(),
        format_args!(
            "{} blob(s), {}",
            report.deferred_blobs.len(),
            format_bytes(report.deferred_size())
        ),
    );
    if show_digests {
        print_blob_list(&report.deferred_blobs);
    }
    if !report.missing_blobs.is_empty() {
        print_status(
            "Missing".red().bold(),
            format_args!("{} referenced blob(s)", report.missing_blobs.len()),
        );
        if show_digests {
            for missing in &report.missing_blobs {
                println!(
                    "  {}  {:?}",
                    missing.digest.to_string().dimmed(),
                    missing.kind
                );
            }
        }
    }
    if !report.invalid_manifests.is_empty() {
        print_status(
            "Invalid".red().bold(),
            format_args!("{} manifest blob(s)", report.invalid_manifests.len()),
        );
        if show_digests {
            for invalid in &report.invalid_manifests {
                println!(
                    "  {}  {:?}: {}",
                    invalid.digest.to_string().dimmed(),
                    invalid.kind,
                    invalid.error
                );
            }
        }
    }
}

fn print_status(label: ColoredString, message: impl std::fmt::Display) {
    println!("{label:>12} {message}");
}

fn print_anonymous_ref(
    name: &str,
    reference: &str,
    digest: impl std::fmt::Display,
    show_digests: bool,
) {
    if show_digests {
        println!(
            "  {}:{}  {}  {}",
            name.dimmed(),
            reference,
            "->".dimmed(),
            digest
        );
    } else {
        println!("  {}:{}", name.dimmed(), reference);
    }
}

fn print_blob_list(blobs: &[GcBlob]) {
    for blob in blobs {
        println!(
            "  {}  {}",
            blob.digest.to_string().dimmed(),
            format_bytes(blob.size)
        );
    }
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
