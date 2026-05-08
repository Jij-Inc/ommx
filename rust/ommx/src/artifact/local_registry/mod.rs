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

use chrono::Utc;

pub use crate::artifact::digest::sha256_digest;
pub(crate) use crate::artifact::digest::{validate_digest, ValidatedDigest};
pub use blob::FileBlobStore;
pub use index::{image_name_repository, SqliteIndexStore};
pub use legacy::{
    import_legacy_local_registry_ref, import_legacy_oci_dir, import_legacy_oci_dir_as_ref,
    import_legacy_oci_dir_as_ref_with_policy, import_legacy_oci_dir_with_policy,
    legacy_local_registry_path, legacy_oci_dir_image_name, legacy_oci_dir_ref,
    migrate_legacy_local_registry, migrate_legacy_local_registry_with_policy,
    LegacyMigrationReport, LegacyOciDirImport, LegacyOciDirRef,
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
