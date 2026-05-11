//! `.ommx` OCI archive → v3 SQLite Local Registry import.
//!
//! Two-stage pipeline glued on top of ocipkg:
//!
//! 1. `Artifact::from_oci_archive(path).load_to(staging_path)` extracts
//!    the archive into a fresh [`tempfile::TempDir`] under the
//!    registry's parent directory. The temp dir is the only on-disk
//!    materialisation of the archive in OCI Image Layout form; it is
//!    dropped when this function returns. v3 has no legacy OCI dir
//!    cache for fresh imports — SQLite is the sole post-import home
//!    of the bytes.
//! 2. [`super::oci_dir::import_oci_dir_as_ref`] reads that temp
//!    directory back, validates manifest / blob digests, and writes
//!    them into the SQLite [`super::super::SqliteIndexStore`] +
//!    [`super::super::FileBlobStore`] without rewriting the manifest.
//!    Ref conflicts (existing SQLite digest ≠ new archive digest) are
//!    surfaced through `RefConflictPolicy` at this stage.
//!
//! The follow-up that drops ocipkg from the import path will replace
//! stage 1 with a native streamer that writes archive bytes straight
//! into [`super::super::FileBlobStore`] and inserts the matching
//! SQLite records, eliminating the on-disk OCI Image Layout
//! intermediate entirely. The public function signature here is what
//! the rest of the SDK depends on, so that swap can land without
//! touching call sites.

use super::super::LocalRegistry;
use super::oci_dir::{import_oci_dir_as_ref, OciDirImport};
use crate::artifact::Artifact;
use anyhow::{Context, Result};
use ocipkg::image::Image;
use std::{path::Path, sync::Arc};

/// Import a `.ommx` OCI archive on disk into the v3 SQLite Local Registry.
///
/// Reads the archive's manifest / config / layer blobs through ocipkg,
/// stages them into a tempdir-backed OCI Image Layout, then imports
/// that directory into SQLite preserving manifest digest. Returns the
/// [`OciDirImport`] outcome reported by the underlying directory
/// import (`Inserted` on first call for this image, `Unchanged` for
/// an idempotent re-import of the same digest, or `Err` for a ref
/// conflict when the new archive's manifest digest differs from the
/// SQLite-recorded one under `KeepExisting` policy).
///
/// The staging tempdir is created under the registry root so the OS
/// rename / copy stays on the same filesystem, and is removed before
/// the function returns; the post-import home of the bytes is the
/// SQLite registry alone.
pub fn import_oci_archive(registry: &Arc<LocalRegistry>, path: &Path) -> Result<OciDirImport> {
    let mut artifact = Artifact::from_oci_archive(path)?;
    let image_name = artifact.get_name()?;

    let staging_parent = registry.root();
    std::fs::create_dir_all(staging_parent).with_context(|| {
        format!(
            "Failed to create registry root {}",
            staging_parent.display()
        )
    })?;
    let temp_holder = tempfile::tempdir_in(staging_parent).with_context(|| {
        format!(
            "Failed to create temp staging dir in {}",
            staging_parent.display()
        )
    })?;
    let staging_path = temp_holder.path().join("staged");
    artifact.load_to(&staging_path)?;

    import_oci_dir_as_ref(
        registry.index(),
        registry.blobs(),
        staging_path,
        &image_name,
    )
}
