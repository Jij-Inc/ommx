//! `.ommx` OCI archive → v3 SQLite Local Registry import.
//!
//! Currently this is a **two-stage pipeline glued on top of ocipkg**:
//!
//! 1. `Artifact::from_oci_archive(path).load_to(legacy_path)` extracts
//!    the archive into a legacy OCI dir under
//!    `registry.root().join(image_name.as_path())` using
//!    `ocipkg::image::copy` and `OciDirBuilder`. Routing through
//!    [`crate::artifact::Artifact::load_to`] (instead of [`Artifact::load`])
//!    keeps the legacy staging dir under the same root as the SQLite
//!    registry — important when the caller opens the registry on a
//!    non-default root.
//! 2. [`super::oci_dir::import_oci_dir_as_ref`] reads that legacy
//!    directory back, validates manifest / blob digests, and writes
//!    them into the SQLite [`super::super::SqliteIndexStore`] +
//!    [`super::super::FileBlobStore`] without rewriting the manifest.
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
use anyhow::Result;
use ocipkg::image::Image;
use std::{path::Path, sync::Arc};

/// Import a `.ommx` OCI archive on disk into the v3 SQLite Local Registry.
///
/// Reads the archive's manifest / config / layer blobs through ocipkg,
/// extracts them into a legacy OCI dir under `registry.root()`, and then
/// imports that directory into SQLite preserving manifest digest.
/// Returns the [`OciDirImport`] outcome reported by the underlying
/// directory import (`Inserted` on first call for this image,
/// `Unchanged` for an idempotent re-import of the same digest).
pub fn import_oci_archive(registry: &Arc<LocalRegistry>, path: &Path) -> Result<OciDirImport> {
    let mut artifact = Artifact::from_oci_archive(path)?;
    let image_name = artifact.get_name()?;
    let legacy_path = registry.root().join(image_name.as_path());
    artifact.load_to(&legacy_path)?;
    import_oci_dir_as_ref(registry.index(), registry.blobs(), legacy_path, &image_name)
}
