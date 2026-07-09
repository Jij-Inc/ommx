pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

use crate::artifact::{media_types, sha256_digest};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Digest, ImageManifest, MediaType};
use std::collections::BTreeMap;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub manifest_digest: Digest,
    pub updated_at: String,
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
    status: String,
    run_count: u64,
    solve_count: u64,
}

impl ExperimentManifestRecord {
    pub(crate) fn from_validated_summary(
        artifact: ArtifactManifestRecord,
        config_json: Vec<u8>,
        status: String,
        run_count: u64,
        solve_count: u64,
    ) -> Result<Self> {
        ensure!(
            artifact.config_digest().as_ref() == sha256_digest(&config_json),
            "Experiment config cache digest mismatch for {}",
            artifact.config_digest()
        );
        Ok(Self {
            artifact,
            config_json,
            status,
            run_count,
            solve_count,
        })
    }

    pub(crate) fn artifact(&self) -> &ArtifactManifestRecord {
        &self.artifact
    }

    pub(crate) fn config_json(&self) -> &[u8] {
        &self.config_json
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn run_count(&self) -> u64 {
        self.run_count
    }

    pub(crate) fn solve_count(&self) -> u64 {
        self.solve_count
    }
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
