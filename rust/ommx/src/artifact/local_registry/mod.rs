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
//!   shared row / ref-update types. [`LocalRegistry`] glues the two stores
//!   into a single addressable unit and contains the publish primitive
//!   used by higher-level commit implementations.
//! - **Import** — `import`. Boundary code that reads external content
//!   in its native form and writes it through [`LocalRegistry`].
//!   Currently `import::oci_dir` (a single OCI Image Layout directory)
//!   and `import::legacy` (a v2 OMMX local registry path/tag tree of
//!   such directories). All imports are identity-preserving: manifest
//!   bytes and digest are stored verbatim. Reformatting an Image
//!   Manifest into an Artifact Manifest is a separate explicit
//!   `convert` operation that produces a new artifact under a new
//!   digest / new ref, intentionally not a side effect of import.
//!
//! Terminology used in this module:
//!
//! Data-model terms:
//!
//! - **Descriptor** is an OCI descriptor. It is a claim about digest,
//!   size, media type, and annotations; by itself it does not prove the
//!   described bytes are present in this registry.
//! - [`StoredDescriptor`] is an OCI descriptor plus the Local Registry
//!   invariant that the described blob exists in this registry's
//!   BlobStore. The invariant is existence in the registry, not
//!   authorship by the current call.
//! - **Unsealed** is the state of a multi-blob object whose component
//!   blobs may already be represented by [`StoredDescriptor`]s, but
//!   whose root manifest blob has not yet been stored. The object is
//!   still being constructed as a whole.
//! - **Sealed** is the state after the root manifest blob has been
//!   stored and represented by `SealedArtifact`.
//! - **Published** is the state where [`SqliteIndexStore`] records a
//!   ref pointing at a `SealedArtifact`.
//!
//! Operation terms:
//!
//! - **Store** belongs to [`FileBlobStore`] / [`LocalRegistry`]. It
//!   writes bytes as a content-addressed blob and yields a
//!   [`StoredDescriptor`] after digest / size verification.
//! - **Seal** stores the root manifest blob for unsealed state and
//!   yields a `SealedArtifact`. It does not write
//!   [`SqliteIndexStore`].
//! - **Publish** belongs to the registry index. Publishing records that
//!   a ref points at a sealed root manifest descriptor in
//!   [`SqliteIndexStore`]. It succeeds for a new ref or an idempotent
//!   same-digest ref, and reports a conflict when the ref already
//!   points at a different digest. It does not write payload blobs.
//! - **Replace** also belongs to the registry index. Replacing moves a
//!   ref to a different sealed root manifest descriptor and reports
//!   the previous digest when one existed.
//! - **Commit** belongs to higher-level mutable objects such as
//!   `ArtifactDraft` and Experiment sessions. A commit seals their
//!   unsealed state into an immutable artifact, and normally publishes
//!   the resulting root descriptor under a ref. APIs that intentionally
//!   move an existing ref expose that as a separate replace operation,
//!   not as stored state on the unsealed object.
//! - **Staged** is not a data-model state in the current design.
//!   Payload blobs are stored eagerly; if a descriptor is kept in
//!   unsealed state for a future commit, it is already stored.

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
    import_legacy_local_registry, import_legacy_local_registry_ref, legacy_local_registry_path,
    replace_legacy_local_registry, replace_legacy_local_registry_ref, LegacyImportReport,
};
pub use import::oci_dir::{
    import_oci_dir, import_oci_dir_as_ref, oci_dir_image_name, oci_dir_ref, replace_oci_dir,
    replace_oci_dir_as_ref, OciDirImport, OciDirRef,
};
#[cfg(feature = "remote-artifact")]
pub use import::remote::pull_image;
pub use index::SqliteIndexStore;
pub(crate) use registry::UnsealedArtifact;
pub use registry::{LocalRegistry, StoredDescriptor};
pub use types::{
    RefRecord, RefUpdate, FILE_BLOB_STORE_DIR_NAME, OCI_IMAGE_REF_NAME_ANNOTATION,
    SQLITE_INDEX_FILE_NAME,
};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
