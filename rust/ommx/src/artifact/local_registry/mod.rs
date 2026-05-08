//! SQLite-backed local registry index and filesystem content store.
//!
//! This module is intentionally independent from the current `ocipkg::OciDir`
//! local-registry layout. The legacy layout remains a read/import source; new
//! local-registry state is represented by an index store plus a CAS blob store.

mod blob;
mod index;
mod legacy;
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
pub use index::{image_name_repository, SqliteIndexStore};
pub use legacy::{
    import_legacy_local_registry, import_legacy_local_registry_ref,
    import_legacy_local_registry_ref_with_policy, import_legacy_local_registry_with_policy,
    import_legacy_oci_dir, import_legacy_oci_dir_as_ref, import_legacy_oci_dir_as_ref_with_policy,
    import_legacy_oci_dir_with_policy, legacy_local_registry_path, legacy_oci_dir_image_name,
    legacy_oci_dir_ref, LegacyImportReport, LegacyOciDirRef,
};
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
