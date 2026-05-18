//! Sealing an experiment session into an immutable OMMX Artifact.

use super::parameter::ParameterTable;
use super::{RecordRef, UnsealedExperimentState};
use super::{
    ANN_ARTIFACT_KIND, ANN_EXPERIMENT_NAME, ANN_EXPERIMENT_SCHEMA, ANN_EXPERIMENT_STATUS,
    ANN_LAYER, ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_INDEX_MEDIA_TYPE, EXPERIMENT_SCHEMA_V1,
    EXPERIMENT_STATUS_FINISHED, LAYER_KIND_INDEX, LAYER_KIND_RUN_ATTRIBUTES,
    LAYER_KIND_RUN_PARAMETERS, RUN_ATTRIBUTES_MEDIA_TYPE, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    LocalRegistry, RefUpdate, StoredDescriptor, UnsealedArtifact,
};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use anyhow::Result;
use oci_spec::image::MediaType;
use serde::Serialize;
use std::collections::HashMap;

impl<'reg> UnsealedExperimentState<'reg> {
    /// Consume the unsealed experiment state and commit it as one
    /// immutable artifact. This is the state-level counterpart of the
    /// public `Experiment::commit(self)` lifecycle operation.
    pub(super) fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let image_name = self.image_name(registry)?;
        let config_descriptor = registry.store_empty_config()?;
        let layers = self.materialize_layers(registry)?;
        let artifact = UnsealedArtifact::new(
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers,
            None,
            manifest_annotations(&self),
        );
        let sealed_artifact = registry.seal_artifact(artifact)?;
        let ref_update = registry.publish_manifest_ref(&image_name, &sealed_artifact)?;
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
            registry,
            image_name,
            sealed_artifact.digest().clone(),
        ))
    }

    /// Collect already-stored record descriptors and materialize the
    /// commit-time aggregate JSON layers.
    fn materialize_layers(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<Vec<StoredDescriptor<'reg>>> {
        let mut layers = self.record_layers();
        layers.extend(self.aggregate_layers(registry)?);
        Ok(layers)
    }

    /// Record layers: experiment space first, then each run's space.
    /// `layers[]` keeps one descriptor per record (digests may repeat
    /// across annotation-distinct layers). The payload bytes were
    /// already written when each record was logged.
    fn record_layers(&self) -> Vec<StoredDescriptor<'reg>> {
        let run_records = self.runs.iter().flat_map(|run| run.records.iter());
        self.records
            .iter()
            .chain(run_records)
            .map(|record| record.descriptor.clone())
            .collect()
    }

    fn aggregate_layers(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<Vec<StoredDescriptor<'reg>>> {
        Ok(vec![
            store_aggregate_json_layer(
                registry,
                RUN_PARAMETERS_MEDIA_TYPE,
                LAYER_KIND_RUN_PARAMETERS,
                &run_parameters_json(self)?,
            )?,
            store_aggregate_json_layer(
                registry,
                RUN_ATTRIBUTES_MEDIA_TYPE,
                LAYER_KIND_RUN_ATTRIBUTES,
                &run_attributes_json(self),
            )?,
            store_aggregate_json_layer(
                registry,
                EXPERIMENT_INDEX_MEDIA_TYPE,
                LAYER_KIND_INDEX,
                &experiment_index_json(self),
            )?,
        ])
    }

    fn image_name(&self, registry: &LocalRegistry) -> Result<ImageRef> {
        match &self.requested_ref {
            Some(image_ref) => Ok(image_ref.clone()),
            None => registry.synthesize_anonymous_image_name(),
        }
    }
}

fn store_aggregate_json_layer<'reg>(
    registry: &'reg LocalRegistry,
    media_type: &str,
    layer_kind: &str,
    value: &impl Serialize,
) -> Result<StoredDescriptor<'reg>> {
    let mut annotations = HashMap::new();
    annotations.insert(ANN_LAYER.to_string(), layer_kind.to_string());
    registry.store_json_layer_blob(MediaType::Other(media_type.to_string()), value, annotations)
}

fn manifest_annotations(state: &UnsealedExperimentState<'_>) -> HashMap<String, String> {
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

#[derive(Serialize)]
struct RunAttributes {
    runs: Vec<RunAttributeRow>,
}

#[derive(Serialize)]
struct RunAttributeRow {
    run_id: u64,
    status: &'static str,
    elapsed_seconds: f64,
}

fn run_attributes_json(state: &UnsealedExperimentState<'_>) -> RunAttributes {
    RunAttributes {
        runs: state
            .runs
            .iter()
            .map(|run| RunAttributeRow {
                run_id: run.run_id,
                status: run.status.as_str(),
                elapsed_seconds: run.elapsed_secs,
            })
            .collect(),
    }
}

fn run_parameters_json(state: &UnsealedExperimentState<'_>) -> Result<ParameterTable> {
    ParameterTable::from_runs(state)
}

#[derive(Serialize)]
struct ExperimentIndex {
    schema: &'static str,
    name: String,
    experiment_records: Vec<RecordIndexEntry>,
    runs: Vec<RunIndexEntry>,
}

#[derive(Serialize)]
struct RunIndexEntry {
    run_id: u64,
    parameter_names: Vec<String>,
    records: Vec<RecordIndexEntry>,
}

#[derive(Serialize)]
struct RecordIndexEntry {
    name: String,
    media_type: String,
    digest: String,
    size: u64,
}

fn experiment_index_json(state: &UnsealedExperimentState<'_>) -> ExperimentIndex {
    ExperimentIndex {
        schema: EXPERIMENT_SCHEMA_V1,
        name: state.name.clone(),
        experiment_records: state.records.iter().map(record_index_entry).collect(),
        runs: state
            .runs
            .iter()
            .map(|run| RunIndexEntry {
                run_id: run.run_id,
                parameter_names: run.parameters.keys().cloned().collect(),
                records: run.records.iter().map(record_index_entry).collect(),
            })
            .collect(),
    }
}

fn record_index_entry(record: &RecordRef<'_>) -> RecordIndexEntry {
    RecordIndexEntry {
        name: record.name.clone(),
        media_type: record.descriptor.media_type().to_string(),
        digest: record.descriptor.digest().to_string(),
        size: record.descriptor.size(),
    }
}
