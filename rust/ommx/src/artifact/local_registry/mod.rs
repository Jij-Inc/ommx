//! v3 OMMX Local Registry.
//!
//! The Local Registry stores artifacts as content-addressed blobs in
//! [`FileBlobStore`] plus index records in [`SqliteIndexStore`]. It
//! does **not** keep anything in OCI Image Layout (`oci-layout` +
//! `index.json` + `blobs/`) form internally; that format is purely an
//! interchange boundary handled in the [`import`] submodule.
//!
//! Two distinct layers live here:
//!
//! - **Storage** — [`index`] / [`blob`] / [`types`] / [`registry`].
//!   The SQLite + filesystem CAS that owns v3 local state, plus the
//!   shared row / policy types. [`LocalRegistry`] glues the two stores
//!   into a single addressable unit and exposes the `publish` primitive
//!   used by `LocalArtifactBuilder`.
//! - **Import** — [`import`]. Boundary code that reads external content
//!   in its native form and writes it through [`LocalRegistry`].
//!   Currently `import::oci_dir` (a single OCI Image Layout directory)
//!   and `import::legacy` (a v2 OMMX local registry path/tag tree of
//!   such directories). All imports are identity-preserving: manifest
//!   bytes and digest are stored verbatim. Reformatting an Image
//!   Manifest into an Artifact Manifest is a separate explicit
//!   `convert` operation that produces a new artifact under a new
//!   digest / new ref, intentionally not a side effect of import.

mod blob;
mod import;
mod index;
mod registry;
mod types;

#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;

pub use crate::artifact::digest::sha256_digest;
pub(crate) use crate::artifact::digest::{validate_digest, ValidatedDigest};
pub use blob::FileBlobStore;
pub use import::archive::{import_oci_archive, inspect_archive, ArchiveInspectView};
pub use import::legacy::{
    import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    legacy_local_registry_path, LegacyImportReport,
};
pub use import::oci_dir::{
    import_oci_dir, import_oci_dir_as_ref, import_oci_dir_as_ref_with_policy,
    import_oci_dir_with_policy, oci_dir_image_name, oci_dir_ref, OciDirImport, OciDirRef,
};
#[cfg(feature = "remote-artifact")]
pub use import::remote::pull_image;
pub use index::{PublishOutcome, SqliteIndexStore};
pub use registry::LocalRegistry;
pub use types::{
    BlobRecord, LayerRecord, ManifestRecord, RefConflictPolicy, RefRecord, RefUpdate,
    BLOB_KIND_BLOB, BLOB_KIND_CONFIG, BLOB_KIND_LAYER, BLOB_KIND_MANIFEST,
    FILE_BLOB_STORE_DIR_NAME, OCI_IMAGE_REF_NAME_ANNOTATION, SQLITE_INDEX_FILE_NAME,
};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Encode an optional annotation map as a stable JSON string.
///
/// Stable (key-sorted) encoding is required because the same annotation
/// map must round-trip to the same bytes regardless of insertion order;
/// otherwise the digest of any blob whose descriptor includes
/// annotations would depend on a HashMap iteration order, and rows in
/// `manifests.annotations_json` / `manifest_layers.annotations_json`
/// would not be comparable across the legacy import path and the v3
/// native build path.
pub(super) fn annotations_json(annotations: Option<&HashMap<String, String>>) -> Result<String> {
    match annotations {
        Some(annotations) => String::from_utf8(crate::artifact::stable_json_bytes(annotations)?)
            .context("Stable JSON bytes are not UTF-8"),
        None => Ok("{}".to_string()),
    }
}
