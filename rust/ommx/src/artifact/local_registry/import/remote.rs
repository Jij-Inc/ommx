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
//! Same two-stage pipeline as [`super::archive`], glued on top of
//! ocipkg:
//!
//! 1. `Artifact::from_remote(image).pull_to(legacy_path)` performs the
//!    OCI Distribution pull and writes the manifest / config / layer
//!    blobs into a legacy OCI dir under
//!    `registry.root().join(image_name.as_path())`. Routing through
//!    [`crate::artifact::Artifact::pull_to`] (instead of [`Artifact::pull`])
//!    keeps the legacy staging dir under the same root as the SQLite
//!    registry — important when the caller opens the registry on a
//!    non-default root.
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

use super::super::LocalRegistry;
use super::oci_dir::{import_oci_dir_as_ref, OciDirImport};
use crate::artifact::Artifact;
use anyhow::Result;
use ocipkg::ImageName;
use std::sync::Arc;

/// Pull `image_name` from its remote registry into the v3 SQLite
/// Local Registry.
///
/// If the legacy OCI dir cache already has the image at
/// `registry.root().join(image_name.as_path())`, the network fetch is
/// skipped (matches the existing `Artifact<Remote>::pull` skip-on-exist
/// behaviour). The legacy entry — whether freshly pulled or already
/// present — is then imported into SQLite preserving manifest digest.
/// Returns the [`OciDirImport`] outcome from the underlying directory
/// import.
pub fn pull_image(registry: &Arc<LocalRegistry>, image_name: &ImageName) -> Result<OciDirImport> {
    let legacy_path = registry.root().join(image_name.as_path());
    if !legacy_path.exists() {
        let mut remote = Artifact::from_remote(image_name.clone())?;
        let _ = remote.pull_to(&legacy_path)?;
    }
    import_oci_dir_as_ref(registry.index(), registry.blobs(), legacy_path, image_name)
}
