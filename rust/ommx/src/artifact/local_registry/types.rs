pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

use crate::artifact::{media_types, sha256_digest};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Digest, ImageManifest, MediaType};
use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub manifest_digest: Digest,
    pub updated_at: String,
}

/// Selects synthetic anonymous refs for Local Registry cleanup.
///
/// Anonymous Artifact refs are always included. Set
/// [`Self::include_experiments`] to also include refs created by anonymous
/// Experiment sessions. [`Self::older_than`] applies to the mutable ref's
/// `updated_at` timestamp; it does not inspect immutable Artifact metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnonymousRefOptions {
    /// Include anonymous Experiment refs in addition to anonymous Artifact refs.
    pub include_experiments: bool,
    /// Include only refs whose last update is at least this old.
    pub older_than: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactManifestRecord {
    manifest_digest: Digest,
    manifest_json: Vec<u8>,
    artifact_type: MediaType,
    config_digest: Digest,
}

impl ArtifactManifestRecord {
    pub(crate) fn from_image_manifest(
        manifest_digest: Digest,
        manifest_json: Vec<u8>,
        manifest: &ImageManifest,
    ) -> Result<Self> {
        ensure!(
            manifest_digest.as_ref() == sha256_digest(&manifest_json),
            "Artifact manifest cache digest mismatch for {}",
            manifest_digest
        );
        let artifact_type = manifest
            .artifact_type()
            .as_ref()
            .context("Validated OMMX image manifest is missing artifactType")?
            .clone();
        ensure!(
            media_types::is_ommx_artifact_type(&artifact_type),
            "Manifest `artifactType` must be one of `{}` or `{}`, got `{}`",
            media_types::V1_ARTIFACT_MEDIA_TYPE,
            media_types::V1_EXPERIMENT_MEDIA_TYPE,
            artifact_type,
        );
        Ok(Self {
            manifest_digest,
            manifest_json,
            artifact_type,
            config_digest: manifest.config().digest().clone(),
        })
    }

    pub(crate) fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }

    pub(crate) fn manifest_json(&self) -> &[u8] {
        &self.manifest_json
    }

    pub(crate) fn artifact_type(&self) -> &MediaType {
        &self.artifact_type
    }

    pub(crate) fn config_digest(&self) -> &Digest {
        &self.config_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExperimentManifestRecord {
    artifact: ArtifactManifestRecord,
    config_json: Vec<u8>,
}

impl ExperimentManifestRecord {
    pub(crate) fn from_validated_config(
        artifact: ArtifactManifestRecord,
        config_json: Vec<u8>,
    ) -> Result<Self> {
        ensure!(
            artifact.config_digest().as_ref() == sha256_digest(&config_json),
            "Experiment config cache digest mismatch for {}",
            artifact.config_digest()
        );
        Ok(Self {
            artifact,
            config_json,
        })
    }

    pub(crate) fn artifact(&self) -> &ArtifactManifestRecord {
        &self.artifact
    }

    pub(crate) fn config_json(&self) -> &[u8] {
        &self.config_json
    }
}

/// Local Registry listing record for an OMMX Artifact.
///
/// Values are reconstructed from a ref and the digest-addressed SQLite copy of
/// its original OCI Manifest JSON. The Manifest bytes are verified against
/// `manifest_digest` before this record is returned. Use `manifest_digest` as
/// the immutable artifact identity; `image_name` is the mutable local registry
/// alias that currently points to it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRefRecord {
    /// Full local registry image reference.
    pub image_name: crate::artifact::ImageRef,
    /// Immutable OCI manifest digest for the Artifact.
    pub manifest_digest: Digest,
    /// RFC 3339 timestamp when the local ref was last updated.
    pub updated_at: String,
    /// OCI Manifest `artifactType` identifying the OMMX Artifact kind.
    pub artifact_type: MediaType,
    /// Immutable digest of the config blob referenced by the Manifest.
    pub config_digest: Digest,
    /// Manifest annotations stored on the Artifact.
    pub annotations: BTreeMap<String, String>,
    /// Complete OCI Manifest JSON stored by `manifest_digest`.
    pub manifest: serde_json::Value,
}

/// Controls generic Artifact catalog listing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArtifactListOptions {
    /// Include Local Registry implementation refs such as Experiment
    /// checkpoints. These refs are hidden from the user-facing catalog by
    /// default.
    pub include_internal: bool,
    /// Fail on the first unreadable Artifact instead of returning the other
    /// records with structured warnings.
    pub strict: bool,
}

/// Controls Experiment catalog listing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExperimentListOptions {
    /// Fail on the first unreadable Experiment instead of returning the other
    /// records with structured warnings.
    pub strict: bool,
}

/// Controls Experiment checkpoint listing behavior.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExperimentCheckpointListOptions {
    /// Included checkpoint lifecycle statuses. An empty vector includes all
    /// checkpoint statuses (`draft`, `failed`, and `interrupted`).
    pub statuses: Vec<crate::experiment::ExperimentStatus>,
    /// Fail on the first unreadable checkpoint instead of returning the other
    /// records with structured warnings.
    pub strict: bool,
}

/// Stage at which an individual Local Registry listing record could not be
/// materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryListWarningStage {
    /// A missing Manifest cache row could not be populated from the CAS.
    ManifestBackfill,
    /// An invalid Manifest cache row was repaired or could not be repaired.
    ManifestCacheRepair,
    /// A missing Experiment Config cache row could not be populated from the CAS.
    ExperimentConfigBackfill,
    /// An invalid Experiment Config cache row was repaired or could not be repaired.
    ExperimentConfigCacheRepair,
    /// A cached Experiment could not be interpreted as a recoverable checkpoint.
    CheckpointProjection,
}

impl RegistryListWarningStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ManifestBackfill => "manifest backfill",
            Self::ManifestCacheRepair => "manifest cache repair",
            Self::ExperimentConfigBackfill => "experiment config backfill",
            Self::ExperimentConfigCacheRepair => "experiment config cache repair",
            Self::CheckpointProjection => "checkpoint projection",
        }
    }
}

impl fmt::Display for RegistryListWarningStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured warning for one Local Registry ref omitted or repaired during a
/// best-effort Local Registry listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryListWarning {
    /// Local Registry ref affected by the warning, preserved even when the
    /// stored value is not a valid OCI image reference.
    pub image_name: String,
    /// Manifest identity targeted by the ref, preserved even when the stored
    /// value is not a valid OCI digest.
    pub manifest_digest: String,
    /// Listing stage that repaired or omitted the ref.
    pub stage: RegistryListWarningStage,
    /// Human-readable validation or repair detail.
    pub message: String,
}

impl fmt::Display for RegistryListWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Local Registry listing issue for {} ({}) during {}: {}",
            self.image_name, self.manifest_digest, self.stage, self.message
        )
    }
}

/// Best-effort Local Registry listing result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryListReport<T> {
    /// Successfully materialized listing records.
    pub records: Vec<T>,
    /// Individual refs repaired or omitted while producing `records`.
    pub warnings: Vec<RegistryListWarning>,
}

/// Local Registry listing record for an Experiment artifact.
///
/// Values are reconstructed from digest-addressed SQLite copies of the
/// original Manifest and Config JSON. Use the `manifest_digest` as the
/// immutable artifact identity; `image_name` is the mutable local registry
/// alias that currently points to it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentRefRecord {
    /// Full local registry image reference.
    pub image_name: crate::artifact::ImageRef,
    /// Immutable OCI manifest digest for the Experiment artifact.
    pub manifest_digest: Digest,
    /// Immutable digest of the Experiment config JSON.
    pub config_digest: Digest,
    /// RFC 3339 timestamp when the local ref was last updated.
    pub updated_at: String,
    /// Experiment lifecycle status stored in the Experiment config.
    pub status: String,
    /// Number of closed runs recorded in the Experiment config.
    pub run_count: u64,
    /// Total number of solves recorded across all runs.
    pub solve_count: u64,
    /// Total number of samplings recorded across all runs.
    pub sampling_count: u64,
    /// Manifest annotations stored on the Experiment artifact.
    pub annotations: BTreeMap<String, String>,
    /// Complete Experiment config JSON stored by `config_digest`.
    ///
    /// The JSON value is returned without projecting project-specific or
    /// adapter-specific fields into the Local Registry schema.
    pub config: serde_json::Value,
}

/// Local Registry listing record for an internal Experiment checkpoint.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentCheckpointRefRecord {
    /// Internal Local Registry ref holding the rolling checkpoint Artifact.
    pub checkpoint_image_name: crate::artifact::ImageRef,
    /// User-facing Experiment image name that this checkpoint can restore.
    pub requested_image_name: crate::artifact::ImageRef,
    /// Immutable OCI manifest digest for the checkpoint Artifact.
    pub manifest_digest: Digest,
    /// Immutable digest of the checkpoint's Experiment Config JSON.
    pub config_digest: Digest,
    /// RFC 3339 timestamp when the internal checkpoint ref was last updated.
    pub updated_at: String,
    /// Checkpoint lifecycle status: `draft`, `failed`, or `interrupted`.
    pub status: String,
    /// Number of closed runs available at this recovery point.
    pub run_count: u64,
    /// Total number of solves recorded across the checkpoint's closed runs.
    pub solve_count: u64,
    /// Total number of samplings recorded across the checkpoint's closed runs.
    pub sampling_count: u64,
    /// Manifest annotations stored on the checkpoint Artifact.
    pub annotations: BTreeMap<String, String>,
    /// Complete Experiment Config JSON stored by `config_digest`.
    pub config: serde_json::Value,
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
