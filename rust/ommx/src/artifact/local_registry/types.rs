pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

use oci_spec::image::{Descriptor, Digest, MediaType};
use std::collections::BTreeMap;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub descriptor: Descriptor,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactManifestRecord {
    pub(crate) manifest_descriptor: Descriptor,
    pub(crate) manifest_json: Vec<u8>,
    pub(crate) manifest_annotations: BTreeMap<String, String>,
    pub(crate) artifact_type: MediaType,
    pub(crate) config_descriptor: Descriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExperimentManifestRecord {
    pub(crate) artifact: ArtifactManifestRecord,
    pub(crate) config_json: Vec<u8>,
    pub(crate) status: String,
    pub(crate) run_count: u64,
    pub(crate) solve_count: u64,
}

/// Local Registry reference summary for an Experiment artifact.
///
/// Values are reconstructed from SQLite manifest/config projections whose rows
/// are written from validated local registry artifacts. Use the
/// `manifest_digest` as the immutable artifact identity; `image_name` is the
/// mutable local registry alias that currently points to it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentRefRecord {
    /// Full local registry image reference.
    pub image_name: crate::artifact::ImageRef,
    /// Immutable OCI manifest digest for the Experiment artifact.
    pub manifest_digest: Digest,
    /// RFC 3339 timestamp when the local ref was last updated.
    pub updated_at: String,
    /// Experiment lifecycle status stored in the Experiment config.
    pub status: String,
    /// Number of closed runs recorded in the Experiment config.
    pub run_count: u64,
    /// Total number of solves recorded across all runs.
    pub solve_count: u64,
    /// Manifest annotations stored on the Experiment artifact.
    pub annotations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefUpdate {
    Inserted,
    Unchanged,
    Replaced {
        previous_manifest_digest: Digest,
    },
    Conflicted {
        existing_manifest_digest: Digest,
        incoming_manifest_digest: Digest,
    },
}
