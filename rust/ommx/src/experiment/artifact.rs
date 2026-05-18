//! Mapping an unsealed Experiment state to an immutable OMMX Artifact.

use super::index::ExperimentIndex;
use super::parameter::RunParameterTable;
use super::UnsealedExperimentState;
use super::{
    ANN_ARTIFACT_KIND, ANN_EXPERIMENT_NAME, ANN_EXPERIMENT_SCHEMA, ANN_EXPERIMENT_STATUS,
    ANN_LAYER, ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_INDEX_MEDIA_TYPE, EXPERIMENT_SCHEMA_V1,
    EXPERIMENT_STATUS_FINISHED, LAYER_KIND_INDEX, LAYER_KIND_RUN_PARAMETERS,
    RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    LocalRegistry, RefUpdate, StoredDescriptor, UnsealedArtifact,
};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

impl<'reg> UnsealedExperimentState<'reg> {
    /// Consume the unsealed experiment state and commit it as one
    /// immutable artifact. This is the state-level counterpart of the
    /// public `Experiment::commit(self)` lifecycle operation.
    pub(super) fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let image_name = self.image_name(registry)?;
        let config_descriptor = registry.store_empty_config()?;
        let layers = ExperimentArtifactLayers::from_state(&self, registry)?.into_descriptors();
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

    fn image_name(&self, registry: &LocalRegistry) -> Result<ImageRef> {
        match &self.requested_ref {
            Some(image_ref) => Ok(image_ref.clone()),
            None => registry.synthesize_anonymous_image_name(),
        }
    }
}

/// OCI layer descriptors that make up the Experiment artifact.
///
/// Record descriptors are already stored when the corresponding
/// `log_*` call returns. Aggregate descriptors are produced during
/// commit from the in-memory run table data.
struct ExperimentArtifactLayers<'reg> {
    descriptors: Vec<StoredDescriptor<'reg>>,
}

impl<'reg> ExperimentArtifactLayers<'reg> {
    fn from_state(
        state: &UnsealedExperimentState<'reg>,
        registry: &'reg LocalRegistry,
    ) -> Result<Self> {
        let mut descriptors = Self::record_descriptors(state);
        descriptors.extend(Self::aggregate_descriptors(state, registry)?);
        Ok(Self { descriptors })
    }

    fn into_descriptors(self) -> Vec<StoredDescriptor<'reg>> {
        self.descriptors
    }

    /// Record layers: experiment space first, then each run's space.
    /// The payload bytes were already written when each record was
    /// logged.
    fn record_descriptors(state: &UnsealedExperimentState<'reg>) -> Vec<StoredDescriptor<'reg>> {
        let run_records = state.runs.iter().flat_map(|run| run.records.iter());
        state
            .records
            .iter()
            .chain(run_records)
            .map(|record| record.descriptor.clone())
            .collect()
    }

    fn aggregate_descriptors(
        state: &UnsealedExperimentState<'reg>,
        registry: &'reg LocalRegistry,
    ) -> Result<Vec<StoredDescriptor<'reg>>> {
        Ok(vec![
            store_aggregate_json_layer(
                registry,
                RUN_PARAMETERS_MEDIA_TYPE,
                LAYER_KIND_RUN_PARAMETERS,
                &RunParameterTable::from_runs(&state.runs)?,
            )?,
            store_aggregate_json_layer(
                registry,
                EXPERIMENT_INDEX_MEDIA_TYPE,
                LAYER_KIND_INDEX,
                &ExperimentIndex::from_state(state),
            )?,
        ])
    }
}

fn store_aggregate_json_layer<'reg>(
    registry: &'reg LocalRegistry,
    media_type: &str,
    layer_kind: &str,
    value: &impl serde::Serialize,
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
