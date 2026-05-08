//! Remote OCI registry → v3 SQLite Local Registry import.
//!
//! Same shape as [`super::archive`]: a two-stage pipeline glued on top
//! of ocipkg.
//!
//! 1. `Artifact::from_remote(image).pull()` performs the OCI
//!    Distribution pull and writes the manifest / config / layer
//!    blobs into the legacy OCI dir at `get_image_dir(image_name)`.
//! 2. [`super::oci_dir::import_oci_dir_as_ref`] reads that legacy
//!    directory back, validates manifest / blob digests, and writes
//!    them into the SQLite [`super::super::SqliteIndexStore`] +
//!    [`super::super::FileBlobStore`].
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

#![cfg(feature = "remote-artifact")]

use super::super::LocalRegistry;
use super::oci_dir::{import_oci_dir_as_ref, OciDirImport};
use crate::artifact::{get_image_dir, Artifact};
use anyhow::Result;
use ocipkg::ImageName;
use std::sync::Arc;

/// Pull `image_name` from its remote registry into the v3 SQLite
/// Local Registry.
///
/// If the legacy OCI dir cache already has the image, the network
/// fetch is skipped (matches the existing `Artifact<Remote>::pull`
/// behaviour). The legacy entry — whether freshly pulled or already
/// present — is then imported into SQLite preserving manifest digest.
/// Returns the [`OciDirImport`] outcome from the underlying directory
/// import.
pub fn pull_image(registry: &Arc<LocalRegistry>, image_name: &ImageName) -> Result<OciDirImport> {
    let legacy_path = get_image_dir(image_name);
    if !legacy_path.exists() {
        let mut remote = Artifact::from_remote(image_name.clone())?;
        let _ = remote.pull()?;
    }
    import_oci_dir_as_ref(registry.index(), registry.blobs(), legacy_path, image_name)
}
