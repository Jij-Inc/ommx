//! v3 OMMX Local Registry.
//!
//! The Local Registry stores artifact bytes as content-addressed blobs
//! in [`FileBlobStore`]. [`SqliteIndexStore`] is the concurrency-safe
//! equivalent of OCI `index.json`: it stores refs and their target
//! manifest descriptors, not a cache of blobs, manifests, or layers.
//!
//! Two distinct layers live here:
//!
//! - **Storage** — `index` / `blob` / `types` / `registry`.
//!   The SQLite + filesystem CAS that owns v3 local state, plus the
//!   shared row / policy types. [`LocalRegistry`] glues the two stores
//!   into a single addressable unit and exposes the `publish` primitive
//!   used by `ArtifactDraft`.
//! - **Import** — `import`. Boundary code that reads external content
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

use chrono::Utc;

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
pub use index::SqliteIndexStore;
pub use registry::LocalRegistry;
pub use types::{
    RefConflictPolicy, RefRecord, RefUpdate, FILE_BLOB_STORE_DIR_NAME,
    OCI_IMAGE_REF_NAME_ANNOTATION, SQLITE_INDEX_FILE_NAME,
};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
