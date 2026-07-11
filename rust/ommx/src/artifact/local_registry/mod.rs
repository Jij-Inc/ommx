//! v3 OMMX Local Registry.
//!
//! The Local Registry stores artifact bytes in a filesystem-backed
//! content-addressed store. Its SQLite index is the concurrency-safe
//! equivalent of OCI `index.json`: it stores refs and their target
//! manifest digests. It also caches the original Manifest and Experiment
//! Config JSON bytes under their content digests for catalog queries; the
//! filesystem CAS remains the source of truth for bytes.
//!
//! Two distinct layers live here:
//!
//! - **Storage** — `index` / `types` / `registry`.
//!   The SQLite + filesystem CAS that owns v3 local state, plus the
//!   shared row / ref-update types. [`LocalRegistry`] glues the two stores
//!   into a single addressable unit and contains the publish primitive
//!   used by higher-level commit implementations.
//! - **Import** — [`LocalRegistry`] methods that read external content
//!   in its native form and write it through the registry. This includes
//!   a single OCI Image Layout directory, a `.ommx` archive, remote OCI
//!   pulls, and a v2 OMMX local registry path/tag tree. All imports are
//!   identity-preserving: manifest bytes and digest are stored verbatim.
//!   Reformatting an Image Manifest into an Artifact Manifest is a
//!   separate explicit `convert` operation that produces a new artifact
//!   under a new digest / new ref, intentionally not a side effect of
//!   import.
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
//!   content-addressed storage. The invariant is existence in the registry, not
//!   authorship by the current call.
//! - **Unsealed** is the state of a multi-blob object whose component
//!   blobs may already be represented by [`StoredDescriptor`]s, but
//!   whose root manifest blob has not yet been stored. The object is
//!   still being constructed as a whole.
//! - **Sealed** is the state after the root manifest blob has been
//!   stored and represented by `SealedArtifact`.
//! - **Published** is the state where the SQLite index records a
//!   ref pointing at a `SealedArtifact`.
//!
//! Operation terms:
//!
//! - **Store** belongs to [`LocalRegistry`]. It writes bytes as a
//!   content-addressed blob and yields a
//!   [`StoredDescriptor`] after digest / size verification.
//! - **Seal** stores the root manifest blob for unsealed state and
//!   yields a `SealedArtifact`. It does not write
//!   the SQLite index.
//! - **Publish** belongs to the registry index. Publishing records that
//!   a ref points at a sealed root manifest digest in the SQLite index.
//!   It succeeds for a new ref or an idempotent
//!   same-digest ref, and reports a conflict when the ref already
//!   points at a different digest. It does not write payload blobs.
//! - **Replace** also belongs to the registry index. Replacing moves a
//!   ref to a different sealed root manifest digest and reports
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

mod index;
mod registry;
mod types;

#[cfg(test)]
mod tests;

use chrono::Utc;

pub use crate::artifact::digest::sha256_digest;
// Crate-visible only because the top-level `artifact` and `experiment`
// modules construct registry-owned manifests; it is not part of the public API.
pub use index::ArtifactRefRecord;
pub(crate) use registry::UnsealedArtifact;
pub use registry::{
    ArchiveInspectView, GcBlob, GcDeleteReport, GcInvalidManifest, GcMissingBlob, GcOptions,
    GcReferenceKind, GcReport, GcRoot, LegacyImportReport, LocalRegistry, OciDirImport, OciDirRef,
    StoredDescriptor, TempLocalRegistry,
};
pub use types::{
    AnonymousRefOptions, ArtifactListOptions, ExperimentCheckpointListOptions,
    ExperimentCheckpointRefRecord, ExperimentListOptions, ExperimentRefRecord, RefRecord,
    RefUpdate, RegistryListReport, RegistryListWarning, RegistryListWarningStage,
    OCI_IMAGE_REF_NAME_ANNOTATION, SQLITE_INDEX_FILE_NAME,
};
pub(crate) use types::{ArtifactManifestRecord, ExperimentManifestRecord};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
