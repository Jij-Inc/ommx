//! Sealing an experiment session into an immutable OMMX Artifact.

use super::model::{ExperimentState, Record};
use super::{
    build_descriptor, ANN_ARTIFACT_KIND, ANN_EXPERIMENT_NAME, ANN_EXPERIMENT_SCHEMA,
    ANN_EXPERIMENT_STATUS, ANN_LAYER, ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_INDEX_MEDIA_TYPE,
    EXPERIMENT_SCHEMA_V1, EXPERIMENT_STATUS_FINISHED, LAYER_KIND_INDEX, LAYER_KIND_RUN_ATTRIBUTES,
    RUN_ATTRIBUTES_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    BlobRecord, LocalRegistry, RefConflictPolicy, RefUpdate, BLOB_KIND_CONFIG,
};
use crate::artifact::{media_types, sha256_digest, stable_json_bytes, LocalArtifact};
use anyhow::Result;
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifestBuilder, MediaType};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

/// Assemble the experiment manifest from the already-staged record
/// blobs plus the commit-time aggregate layers, and publish it to the
/// Local Registry.
pub(super) fn build_and_publish(
    registry: &Arc<LocalRegistry>,
    state: &ExperimentState,
) -> Result<LocalArtifact> {
    let mut layers: Vec<Descriptor> = Vec::new();
    let mut blob_records: Vec<BlobRecord> = Vec::new();
    let mut seen_digests: HashSet<String> = HashSet::new();

    // Record layers: experiment space first, then each run's space.
    // `layers[]` keeps one descriptor per record (digests may repeat
    // across annotation-distinct layers); `blob_records` is de-duped
    // by digest, since the BlobStore shares identical payloads.
    let run_records = state.runs.iter().flat_map(|run| run.records.iter());
    for record in state.records.iter().chain(run_records) {
        layers.push(record.descriptor.clone());
        if seen_digests.insert(record.blob.digest.clone()) {
            blob_records.push(record.blob.clone());
        }
    }

    // Aggregate layers, materialised at commit time.
    let run_attributes = serde_json::to_vec(&run_attributes_json(state))
        .map_err(|e| crate::error!("Failed to encode run attributes JSON: {e}"))?;
    let (descriptor, blob) = stage_aggregate_layer(
        registry,
        RUN_ATTRIBUTES_MEDIA_TYPE,
        LAYER_KIND_RUN_ATTRIBUTES,
        &run_attributes,
    )?;
    layers.push(descriptor);
    if seen_digests.insert(blob.digest.clone()) {
        blob_records.push(blob);
    }

    let index = serde_json::to_vec(&experiment_index_json(state))
        .map_err(|e| crate::error!("Failed to encode experiment index JSON: {e}"))?;
    let (descriptor, blob) = stage_aggregate_layer(
        registry,
        EXPERIMENT_INDEX_MEDIA_TYPE,
        LAYER_KIND_INDEX,
        &index,
    )?;
    layers.push(descriptor);
    if seen_digests.insert(blob.digest.clone()) {
        blob_records.push(blob);
    }

    // OCI 1.1 empty config blob. Built without an `annotations` field
    // to match `LocalArtifactBuilder::stage`.
    let empty_config_bytes = media_types::OCI_EMPTY_CONFIG_BYTES.to_vec();
    let config_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::EmptyJSON)
        .digest(
            Digest::from_str(&sha256_digest(&empty_config_bytes))
                .map_err(|e| crate::error!("Failed to parse empty config digest: {e}"))?,
        )
        .size(empty_config_bytes.len() as u64)
        .build()
        .map_err(|e| crate::error!("Failed to build empty config descriptor: {e}"))?;
    let mut config_blob = registry.blobs().put_bytes(&empty_config_bytes)?;
    config_blob.media_type = Some(MediaType::EmptyJSON.to_string());
    config_blob.kind = BLOB_KIND_CONFIG.to_string();
    if seen_digests.insert(config_blob.digest.clone()) {
        blob_records.push(config_blob);
    }

    let manifest = ImageManifestBuilder::default()
        .schema_version(2u32)
        .artifact_type(MediaType::Other(
            media_types::V1_ARTIFACT_MEDIA_TYPE.to_string(),
        ))
        .config(config_descriptor)
        .layers(layers)
        .annotations(manifest_annotations(state))
        .build()
        .map_err(|e| crate::error!("Failed to build experiment OCI image manifest: {e}"))?;
    let manifest_bytes = stable_json_bytes(&manifest)?;
    let manifest_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(
            Digest::from_str(&sha256_digest(&manifest_bytes))
                .map_err(|e| crate::error!("Failed to parse manifest digest: {e}"))?,
        )
        .size(manifest_bytes.len() as u64)
        .build()
        .map_err(|e| crate::error!("Failed to build manifest descriptor: {e}"))?;

    let image_name = match &state.requested_ref {
        Some(image_ref) => image_ref.clone(),
        None => registry.synthesize_anonymous_image_name()?,
    };

    let ref_update = registry.publish_prestaged_artifact_manifest(
        &image_name,
        &manifest,
        &manifest_descriptor,
        &manifest_bytes,
        &blob_records,
        RefConflictPolicy::KeepExisting,
    )?;
    if let RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    } = ref_update
    {
        crate::bail!(
            "Local registry ref {image_name} already points to {existing_manifest_digest}; \
             experiment manifest {incoming_manifest_digest} was not published"
        );
    }

    Ok(LocalArtifact::from_parts(
        Arc::clone(registry),
        image_name,
        manifest_descriptor.digest().to_string(),
    ))
}

/// CAS-write a commit-time aggregate JSON layer and return its
/// descriptor (with the `org.ommx.experiment.layer` annotation) plus
/// blob record.
fn stage_aggregate_layer(
    registry: &LocalRegistry,
    media_type: &str,
    layer_kind: &str,
    bytes: &[u8],
) -> Result<(Descriptor, BlobRecord)> {
    let mut blob = registry.blobs().put_bytes(bytes)?;
    blob.media_type = Some(media_type.to_string());
    let mut annotations = HashMap::new();
    annotations.insert(ANN_LAYER.to_string(), layer_kind.to_string());
    let descriptor =
        build_descriptor(MediaType::Other(media_type.to_string()), &blob, annotations)?;
    Ok((descriptor, blob))
}

fn manifest_annotations(state: &ExperimentState) -> HashMap<String, String> {
    HashMap::from([
        (
            ANN_ARTIFACT_KIND.to_string(),
            ARTIFACT_KIND_EXPERIMENT.to_string(),
        ),
        (
            ANN_EXPERIMENT_SCHEMA.to_string(),
            EXPERIMENT_SCHEMA_V1.to_string(),
        ),
        (ANN_EXPERIMENT_NAME.to_string(), state.name.clone()),
        (
            ANN_EXPERIMENT_STATUS.to_string(),
            EXPERIMENT_STATUS_FINISHED.to_string(),
        ),
    ])
}

fn run_attributes_json(state: &ExperimentState) -> serde_json::Value {
    json!({
        "runs": state
            .runs
            .iter()
            .map(|run| json!({
                "run_id": run.run_id,
                "status": run.status.as_str(),
                "elapsed_seconds": run.elapsed_secs,
            }))
            .collect::<Vec<_>>(),
    })
}

fn experiment_index_json(state: &ExperimentState) -> serde_json::Value {
    json!({
        "schema": EXPERIMENT_SCHEMA_V1,
        "name": state.name,
        "experiment_records": state
            .records
            .iter()
            .map(record_index_entry)
            .collect::<Vec<_>>(),
        "runs": state
            .runs
            .iter()
            .map(|run| json!({
                "run_id": run.run_id,
                "records": run.records.iter().map(record_index_entry).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    })
}

fn record_index_entry(record: &Record) -> serde_json::Value {
    json!({
        "name": record.name,
        "media_type": record.descriptor.media_type().to_string(),
        "digest": record.descriptor.digest().to_string(),
        "size": record.descriptor.size(),
    })
}
