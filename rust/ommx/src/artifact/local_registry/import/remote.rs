//! Remote OCI registry → v3 SQLite Local Registry import.
//!
//! ## Naming note: `pull_image` vs `import_*`
//!
//! The other import sources expose `import_<noun>` entry points
//! (`import_oci_dir`, `import_oci_archive`, `import_legacy_local_registry`).
//! This module deliberately names its entry point [`pull_image`]
//! instead, mirroring the OCI Distribution Spec verb and the
//! surrounding ecosystem (`docker pull`, `oras pull`, `crane pull`).
//! Renaming it to `import_remote` would lose the OCI-domain signal
//! that the operation is a network fetch with the standard pull
//! semantics; the `import` namespace it lives in already conveys
//! that the result lands in the v3 registry.
//!
//! ## Implementation shape
//!
//! Two-stage pipeline glued on top of ocipkg:
//!
//! 1. `Artifact::from_remote(image).pull_to(staging_path)` performs
//!    the OCI Distribution pull into a [`tempfile::TempDir`] under
//!    the registry root. The temp dir is the only on-disk
//!    materialisation of the pull in OCI Image Layout form; it is
//!    dropped when this function returns. v3 has no legacy OCI dir
//!    cache for fresh pulls — SQLite is the sole post-import home
//!    of the bytes.
//! 2. [`super::oci_dir::import_oci_dir_as_ref`] reads that temp
//!    directory back, validates manifest / blob digests, and writes
//!    them into the SQLite [`super::super::SqliteIndexStore`] +
//!    [`super::super::FileBlobStore`].
//!
//! The pre-pull SQLite check short-circuits the network fetch when
//! the registry already resolves `image_name` to a manifest digest:
//! the function returns an [`OciDirImport`] with
//! [`super::super::RefUpdate::Unchanged`] without touching the
//! network. The same cache-hit semantics that the v2-era legacy dir
//! cache offered, now expressed against the canonical SQLite ref
//! store.
//!
//! Feature-gated behind `remote-artifact` because `Artifact::from_remote`
//! is, and because this is the only place in `local_registry` that
//! touches the network.
//!
//! The follow-up that replaces ocipkg with an external OCI distribution
//! crate will swap stage 1 for a native pull that streams blobs
//! straight into [`super::super::FileBlobStore`]. The public signature
//! is the SDK's contract, so that swap can land without touching
//! `bin/ommx.rs` or the Python entry points.

use super::super::{LocalRegistry, RefUpdate};
use super::oci_dir::{import_oci_dir_as_ref, OciDirImport};
use crate::artifact::Artifact;
use anyhow::{Context, Result};
use ocipkg::ImageName;
use std::sync::Arc;

/// Pull `image_name` from its remote registry into the v3 SQLite
/// Local Registry.
///
/// If the registry already resolves `image_name` to a manifest digest,
/// the network fetch is skipped and the function returns an
/// [`OciDirImport`] with [`RefUpdate::Unchanged`]. Otherwise the image
/// is pulled into a tempdir-backed OCI Image Layout under the
/// registry root and then imported into SQLite preserving manifest
/// digest. The tempdir is removed before the function returns; the
/// post-import home of the bytes is the SQLite registry alone.
///
/// If two threads race the first miss for the same image, both pull
/// (content-addressed, so the bytes match) into separate temp dirs;
/// the import side de-duplicates through SQLite's
/// `publish_artifact_atomic`, so the second writer sees `Unchanged`.
pub fn pull_image(registry: &Arc<LocalRegistry>, image_name: &ImageName) -> Result<OciDirImport> {
    if let Some(manifest_digest) = registry.index().resolve_image_name(image_name)? {
        return Ok(OciDirImport {
            manifest_digest,
            image_name: Some(image_name.clone()),
            ref_update: Some(RefUpdate::Unchanged),
        });
    }

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
    let mut remote = Artifact::from_remote(image_name.clone())?;
    let _ = remote.pull_to(&staging_path)?;
    import_oci_dir_as_ref(registry.index(), registry.blobs(), staging_path, image_name)
}
