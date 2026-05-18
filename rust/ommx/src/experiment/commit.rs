//! Sealing an experiment session into an immutable OMMX Artifact.

use super::model::{ParameterValue, RecordRef, UnsealedExperimentState};
use super::{
    build_descriptor, ANN_ARTIFACT_KIND, ANN_EXPERIMENT_NAME, ANN_EXPERIMENT_SCHEMA,
    ANN_EXPERIMENT_STATUS, ANN_LAYER, ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_INDEX_MEDIA_TYPE,
    EXPERIMENT_SCHEMA_V1, EXPERIMENT_STATUS_FINISHED, LAYER_KIND_INDEX, LAYER_KIND_RUN_ATTRIBUTES,
    LAYER_KIND_RUN_PARAMETERS, RUN_ATTRIBUTES_MEDIA_TYPE, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    LocalRegistry, RefUpdate, StoredDescriptor, UnsealedArtifact,
};
use crate::artifact::{media_types, sha256_digest, ImageRef, LocalArtifact};
use anyhow::Result;
use oci_spec::image::{DescriptorBuilder, Digest, MediaType};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

/// Commit an unsealed experiment state as one immutable artifact.
pub(super) fn commit_experiment_state<'reg>(
    registry: &'reg LocalRegistry,
    state: UnsealedExperimentState<'reg>,
) -> Result<LocalArtifact<'reg>> {
    state.commit(registry)
}

impl<'reg> UnsealedExperimentState<'reg> {
    /// Consume the unsealed experiment state and commit it as one
    /// immutable artifact. This is the state-level counterpart of the
    /// public `Experiment::commit(self)` lifecycle operation.
    pub(super) fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let image_name = self.image_name(registry)?;
        let artifact = self.into_unsealed_artifact(registry)?;
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

    /// Materialize commit-time aggregate layers and assemble the
    /// unsealed root artifact. This stores component blobs but does not
    /// create the root manifest blob and does not update any image ref.
    fn into_unsealed_artifact(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<UnsealedArtifact<'reg>> {
        let mut layers = Vec::new();

        // Record layers: experiment space first, then each run's
        // space. `layers[]` keeps one descriptor per record (digests
        // may repeat across annotation-distinct layers). The payload
        // bytes were already written when each record was logged.
        let run_records = self.runs.iter().flat_map(|run| run.records.iter());
        for record in self.records.iter().chain(run_records) {
            layers.push(record.descriptor.clone());
        }

        // Aggregate layers, materialised at commit time.
        let run_parameters = serde_json::to_vec(&run_parameters_json(self)?)
            .map_err(|e| crate::error!("Failed to encode run parameters JSON: {e}"))?;
        let descriptor = store_aggregate_layer(
            registry,
            RUN_PARAMETERS_MEDIA_TYPE,
            LAYER_KIND_RUN_PARAMETERS,
            &run_parameters,
        )?;
        layers.push(descriptor);

        let run_attributes = serde_json::to_vec(&run_attributes_json(&self))
            .map_err(|e| crate::error!("Failed to encode run attributes JSON: {e}"))?;
        let descriptor = store_aggregate_layer(
            registry,
            RUN_ATTRIBUTES_MEDIA_TYPE,
            LAYER_KIND_RUN_ATTRIBUTES,
            &run_attributes,
        )?;
        layers.push(descriptor);

        let index = serde_json::to_vec(&experiment_index_json(&self))
            .map_err(|e| crate::error!("Failed to encode experiment index JSON: {e}"))?;
        let descriptor = store_aggregate_layer(
            registry,
            EXPERIMENT_INDEX_MEDIA_TYPE,
            LAYER_KIND_INDEX,
            &index,
        )?;
        layers.push(descriptor);

        // OCI 1.1 empty config blob. Built without an `annotations`
        // field to match `ArtifactDraft`'s manifest shape.
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
        let config_descriptor = registry.store_blob(config_descriptor, &empty_config_bytes)?;

        Ok(UnsealedArtifact::new(
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers,
            None,
            manifest_annotations(&self),
        ))
    }

    fn image_name(&self, registry: &LocalRegistry) -> Result<ImageRef> {
        match &self.requested_ref {
            Some(image_ref) => Ok(image_ref.clone()),
            None => registry.synthesize_anonymous_image_name(),
        }
    }
}

/// Store a commit-time aggregate JSON layer and return its
/// descriptor (with the `org.ommx.experiment.layer` annotation).
fn store_aggregate_layer<'reg>(
    registry: &'reg LocalRegistry,
    media_type: &str,
    layer_kind: &str,
    bytes: &[u8],
) -> Result<StoredDescriptor<'reg>> {
    let digest = Digest::from_str(&sha256_digest(bytes))
        .map_err(|e| crate::error!("Failed to parse aggregate layer digest: {e}"))?;
    let mut annotations = HashMap::new();
    annotations.insert(ANN_LAYER.to_string(), layer_kind.to_string());
    let descriptor = build_descriptor(
        MediaType::Other(media_type.to_string()),
        &digest,
        bytes.len() as u64,
        annotations,
    )?;
    registry.store_blob(descriptor, bytes)
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

fn run_attributes_json(state: &UnsealedExperimentState<'_>) -> serde_json::Value {
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

fn run_parameters_json(state: &UnsealedExperimentState<'_>) -> Result<serde_json::Value> {
    let table = ParameterTable::from_runs(state)?;
    Ok(table.to_json())
}

struct ParameterTable {
    columns: BTreeMap<String, ParameterColumn>,
}

impl ParameterTable {
    fn from_runs(state: &UnsealedExperimentState<'_>) -> Result<Self> {
        let mut columns = BTreeMap::new();
        for run in &state.runs {
            for (name, value) in &run.parameters {
                columns
                    .entry(name.clone())
                    .or_insert_with(|| ParameterColumn::from_value(value))
                    .insert(name, run.run_id, value)?;
            }
        }
        Ok(Self { columns })
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "columns": self
                .columns
                .iter()
                .map(|(name, column)| (name.clone(), column.to_json()))
                .collect::<serde_json::Map<_, _>>(),
        })
    }
}

enum ParameterColumn {
    Bool(BTreeMap<u64, bool>),
    Int(BTreeMap<u64, i64>),
    Float(BTreeMap<u64, f64>),
    String(BTreeMap<u64, String>),
}

impl ParameterColumn {
    fn from_value(value: &ParameterValue) -> Self {
        match value {
            ParameterValue::Bool(_) => Self::Bool(BTreeMap::new()),
            ParameterValue::Int(_) => Self::Int(BTreeMap::new()),
            ParameterValue::Float(_) => Self::Float(BTreeMap::new()),
            ParameterValue::String(_) => Self::String(BTreeMap::new()),
        }
    }

    fn insert(&mut self, name: &str, run_id: u64, value: &ParameterValue) -> Result<()> {
        match (self, value) {
            (Self::Bool(values), ParameterValue::Bool(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (Self::Int(values), ParameterValue::Int(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (column @ Self::Int(_), ParameterValue::Float(value)) => {
                let mut values = match std::mem::replace(column, Self::Float(BTreeMap::new())) {
                    Self::Int(values) => values
                        .into_iter()
                        .map(|(run_id, value)| (run_id, value as f64))
                        .collect::<BTreeMap<_, _>>(),
                    _ => unreachable!(),
                };
                values.insert(run_id, *value);
                *column = Self::Float(values);
                Ok(())
            }
            (Self::Float(values), ParameterValue::Int(value)) => {
                values.insert(run_id, *value as f64);
                Ok(())
            }
            (Self::Float(values), ParameterValue::Float(value)) => {
                values.insert(run_id, *value);
                Ok(())
            }
            (Self::String(values), ParameterValue::String(value)) => {
                values.insert(run_id, value.clone());
                Ok(())
            }
            (column, value) => {
                crate::bail!(
                    "Run parameter `{name}` has mixed column types: existing {}, incoming {}",
                    column.type_name(),
                    value.type_name()
                )
            }
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int64",
            Self::Float(_) => "float64",
            Self::String(_) => "string",
        }
    }

    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Bool(values) => json!({
                "type": self.type_name(),
                "values": values,
            }),
            Self::Int(values) => json!({
                "type": self.type_name(),
                "values": values,
            }),
            Self::Float(values) => json!({
                "type": self.type_name(),
                "values": values,
            }),
            Self::String(values) => json!({
                "type": self.type_name(),
                "values": values,
            }),
        }
    }
}

impl ParameterValue {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int64",
            Self::Float(_) => "float64",
            Self::String(_) => "string",
        }
    }
}

fn experiment_index_json(state: &UnsealedExperimentState<'_>) -> serde_json::Value {
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
                "parameter_names": run.parameters.keys().collect::<Vec<_>>(),
                "records": run.records.iter().map(record_index_entry).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    })
}

fn record_index_entry(record: &RecordRef<'_>) -> serde_json::Value {
    json!({
        "name": record.name,
        "media_type": record.descriptor.media_type().to_string(),
        "digest": record.descriptor.digest().to_string(),
        "size": record.descriptor.size(),
    })
}
