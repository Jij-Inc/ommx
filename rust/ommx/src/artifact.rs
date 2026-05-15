//! OMMX Artifact storage and exchange.
//!
//! This module deliberately separates data-model states from API
//! lifecycle operations:
//!
//! Data-model terms:
//!
//! - **Descriptor** is the OCI descriptor itself. It states digest,
//!   size, media type, and annotations, but does not prove that the
//!   described bytes exist in any OMMX Local Registry.
//! - **Stored** is a Local Registry storage invariant. A
//!   [`local_registry::StoredDescriptor`] means the descriptor's digest
//!   has corresponding bytes in that Local Registry's BlobStore. It
//!   does not mean "this call wrote the bytes"; an already-present CAS
//!   blob satisfies the same invariant.
//! - **Unsealed** is the data-model state of a multi-blob object whose
//!   component blobs may already be stored, but whose root manifest has
//!   not yet been stored. Unsealed state can still change.
//! - **Sealed** is the data-model state after the root manifest bytes
//!   have been stored and a root `StoredDescriptor` exists. The sealed
//!   artifact content is immutable and addressable by digest.
//! - **Published** is the registry-index state where a ref points at a
//!   sealed root manifest descriptor.
//!
//! API and operation terms:
//!
//! - **Draft** is an API lifecycle term for a mutable SDK-side object
//!   being edited by the caller. [`ArtifactDraft`] owns unsealed
//!   artifact state.
//! - **Store** is the Local Registry / BlobStore operation that writes
//!   bytes as a content-addressed blob and yields a
//!   `StoredDescriptor`.
//! - **Seal** is the data-model operation that creates and stores the
//!   root manifest blob for unsealed state, yielding the root
//!   `StoredDescriptor`. It does not update a ref.
//! - **Publish** is the Local Registry / index operation that points a
//!   ref at a sealed root manifest descriptor. It is not a payload
//!   blob write.
//! - **Commit** is the Artifact / Experiment lifecycle operation that
//!   moves a draft from unsealed state to a sealed, normally published,
//!   [`LocalArtifact`]. Public SDK code should describe this operation
//!   as commit.
//! - **Import** is a boundary operation that reads an external source
//!   such as an OCI directory, archive, or remote registry and stores
//!   the referenced bytes in the Local Registry while preserving the
//!   source identity.
//!
//! In this model, `add_*` methods write payload bytes to the Local
//! Registry immediately and return `StoredDescriptor`s. `commit()`
//! then creates and stores the manifest blob and updates the registry
//! ref.

mod annotations;
mod config;
pub mod digest;
mod image_ref;
pub mod local_registry;
mod manifest;
pub mod media_types;
#[cfg(feature = "remote-artifact")]
mod push;
#[cfg(feature = "remote-artifact")]
mod remote_transport;
mod save;
pub use annotations::*;
pub use config::*;
pub use digest::sha256_digest;
pub use image_ref::ImageRef;
pub(crate) use manifest::anonymous_artifact_image_name;
pub(crate) use manifest::stable_json_bytes;
pub use manifest::{
    is_anonymous_artifact_ref_name, is_anonymous_artifact_tag, ArtifactDraft, LocalArtifact,
    LocalManifest,
};
pub use media_types::OCI_IMAGE_MANIFEST_MEDIA_TYPE;

use anyhow::{Context, Result};
use oci_spec::image::ImageManifest;

#[cfg(feature = "remote-artifact")]
use crate::artifact::remote_transport::RemoteTransport;
#[cfg(feature = "remote-artifact")]
use oci_client::RegistryOperation;
use std::path::Path;
use std::{env, path::PathBuf, sync::OnceLock};

/// Global storage for the local registry root path
static LOCAL_REGISTRY_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Set the root directory for OMMX local registry
///
/// See [`get_local_registry_root`] for details.
///
pub fn set_local_registry_root(path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    LOCAL_REGISTRY_ROOT.set(path.clone()).map_err(|path| {
        anyhow::anyhow!(
            "Local registry root has already been set: {}",
            path.display()
        )
    })?;
    tracing::info!("Local registry root set via API: {}", path.display());
    Ok(())
}

/// Get the root directory for OMMX local registry
///
/// - Once the root is set, it is immutable for the lifetime of the program.
/// - You can set it via [`set_local_registry_root`] function before calling this.
/// - If this is called without calling [`set_local_registry_root`],
///   - It will check the `OMMX_LOCAL_REGISTRY_ROOT` environment variable.
///   - If the environment variable is not set, it will use the default project data directory.
/// - The root directory is **NOT** created automatically by this function.
///
pub fn get_local_registry_root() -> &'static Path {
    LOCAL_REGISTRY_ROOT.get_or_init(|| {
        // Try environment variable first
        let path = if let Ok(custom_dir) = env::var("OMMX_LOCAL_REGISTRY_ROOT") {
            let path = PathBuf::from(custom_dir);
            tracing::info!(
                "Local registry root initialized from OMMX_LOCAL_REGISTRY_ROOT: {}",
                path.display()
            );
            path
        } else {
            let path = directories::ProjectDirs::from("org", "ommx", "ommx")
                .expect("Failed to get project directories")
                .data_dir()
                .to_path_buf();
            tracing::info!(
                "Local registry root initialized to default: {}",
                path.display()
            );
            path
        };
        path
    })
}

#[deprecated(note = "Use get_local_registry_root instead")]
pub fn data_dir() -> Result<PathBuf> {
    let path = get_local_registry_root().to_path_buf();
    if !path.exists() {
        std::fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create data directory: {}", path.display()))?;
    }
    Ok(path)
}

pub fn ghcr(org: &str, repo: &str, name: &str, tag: &str) -> Result<ImageRef> {
    ImageRef::parse(&format!(
        "ghcr.io/{}/{}/{}:{}",
        org.to_lowercase(),
        repo.to_lowercase(),
        name.to_lowercase(),
        tag
    ))
}

/// Pull only the manifest for `image_name` from its remote registry,
/// without populating the v3 SQLite Local Registry. Used by CLI
/// `ommx inspect <remote-ref>` so the user can read what is on the
/// other side of a ref without committing to a full pull. For the
/// full pull-into-registry flow use [`local_registry::pull_image`].
///
/// Credentials are resolved by `remote_transport::RemoteTransport`'s
/// three-tier chain (env override → `~/.docker/config.json` →
/// anonymous), matching every other network call on the SDK.
#[cfg(feature = "remote-artifact")]
pub fn fetch_remote_manifest(image_name: &ImageRef) -> Result<ImageManifest> {
    let transport = RemoteTransport::new(image_name)?;
    transport.auth_for(image_name, RegistryOperation::Pull)?;
    let (manifest_bytes, _digest) =
        transport.pull_manifest_raw(image_name, &[OCI_IMAGE_MANIFEST_MEDIA_TYPE])?;
    serde_json::from_slice(&manifest_bytes)
        .context("Failed to parse OCI image manifest from the remote registry")
}

/// Get all images stored in the local registry.
pub fn get_images() -> Result<Vec<ImageRef>> {
    let root = get_local_registry_root();
    let registry = local_registry::LocalRegistry::open(root)?;
    registry
        .index()
        .list_refs(None)?
        .into_iter()
        .map(|r| ImageRef::from_repository_and_reference(&r.name, &r.reference))
        .collect()
}

// v3 artifact entry points:
//   - Archive ingest: `local_registry::import_oci_archive(path)`
//   - OCI Image Layout directory ingest: `local_registry::import_oci_dir(path)`
//   - Remote pull into SQLite: `local_registry::pull_image(name)`
//   - Commit into SQLite: `ArtifactDraft::new(...)?.commit()`
//   - Export to archive file: `LocalArtifact::save(path)`
// Image-ref parsing for these entry points goes through [`ImageRef`].
