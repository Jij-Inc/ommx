//! `.ommx` OCI archive → v3 SQLite Local Registry import.
//!
//! Currently this is a **two-stage pipeline glued on top of ocipkg**:
//!
//! 1. `Artifact::from_oci_archive(path).load_to(staging_path)` extracts
//!    the archive into a fresh sibling temp dir, which is then
//!    atomically renamed to the legacy OCI dir at
//!    `registry.root().join(image_name.as_path())`. Routing through
//!    [`crate::artifact::Artifact::load_to`] (instead of [`Artifact::load`])
//!    keeps the legacy staging dir under the same root as the SQLite
//!    registry — important when the caller opens the registry on a
//!    non-default root. Staging into a temp dir means a reader never
//!    observes a half-written `legacy_path`, and re-extracting on
//!    every import means a different archive published under the
//!    same image name is not silently shadowed by a stale dir from a
//!    prior import.
//! 2. [`super::oci_dir::import_oci_dir_as_ref`] reads that legacy
//!    directory back, validates manifest / blob digests, and writes
//!    them into the SQLite [`super::super::SqliteIndexStore`] +
//!    [`super::super::FileBlobStore`] without rewriting the manifest.
//!    Ref conflicts (existing SQLite digest ≠ new archive digest) are
//!    surfaced through `RefConflictPolicy` at this stage.
//!
//! The legacy OCI dir is left in place because `ommx push` / `save`
//! and the Python archive read path still consume it. Until those
//! callers are ported to read directly from the SQLite registry, this
//! module stays as the single chokepoint where archive bytes enter
//! the v3 store.
//!
//! The follow-up that drops ocipkg from this PR's scope will replace
//! the inner two stages with a native v3 path that streams archive
//! bytes straight into [`super::super::FileBlobStore`] and inserts
//! the matching SQLite records, bypassing the legacy dir entirely.
//! The public function signature here is what the rest of the SDK
//! depends on, so that swap can land without touching call sites.

use super::super::LocalRegistry;
use super::oci_dir::{import_oci_dir_as_ref, OciDirImport};
use crate::artifact::Artifact;
use anyhow::{Context, Result};
use ocipkg::image::Image;
use std::{fs, path::Path, sync::Arc};

/// Import a `.ommx` OCI archive on disk into the v3 SQLite Local Registry.
///
/// Reads the archive's manifest / config / layer blobs through ocipkg,
/// extracts them into a legacy OCI dir under `registry.root()`, and then
/// imports that directory into SQLite preserving manifest digest.
/// Returns the [`OciDirImport`] outcome reported by the underlying
/// directory import (`Inserted` on first call for this image,
/// `Unchanged` for an idempotent re-import of the same digest,
/// or `Err` for a ref conflict when the new archive's manifest digest
/// differs from the SQLite-recorded one under `KeepExisting` policy).
pub fn import_oci_archive(registry: &Arc<LocalRegistry>, path: &Path) -> Result<OciDirImport> {
    let mut artifact = Artifact::from_oci_archive(path)?;
    let image_name = artifact.get_name()?;
    let legacy_path = registry.root().join(image_name.as_path());

    // Stage every archive into a fresh temp dir so a stale legacy dir
    // from a prior import of a *different* archive can't silently
    // shadow the requested bytes, and a reader never observes a half-
    // written `legacy_path`. The promote step below clears any
    // existing legacy dir and renames the staged dir into place;
    // POSIX rename requires the destination directory (if any) to be
    // empty, so the clear-then-rename is two syscalls rather than a
    // single atomic step. Single-process CLI / single-threaded Python
    // wrapper are the only callers today; SQLite remains the consistency
    // root.
    let parent = legacy_path
        .parent()
        .context("legacy_path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create parent dir {}", parent.display()))?;
    let temp_holder = tempfile::tempdir_in(parent)
        .with_context(|| format!("Failed to create temp staging dir in {}", parent.display()))?;
    let staging_path = temp_holder.path().join("staged");
    artifact.load_to(&staging_path)?;

    if legacy_path.exists() {
        fs::remove_dir_all(&legacy_path).with_context(|| {
            format!(
                "Failed to clear stale legacy dir at {}",
                legacy_path.display()
            )
        })?;
    }
    fs::rename(&staging_path, &legacy_path).with_context(|| {
        format!(
            "Failed to publish staged archive contents to {}",
            legacy_path.display()
        )
    })?;

    import_oci_dir_as_ref(registry.index(), registry.blobs(), legacy_path, &image_name)
}
