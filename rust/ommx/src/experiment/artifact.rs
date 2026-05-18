//! Mapping an unsealed Experiment state to an immutable OMMX Artifact.

use super::parameter::RunParameterTable;
use super::UnsealedExperimentState;
use super::{
    ANN_ARTIFACT_KIND, ANN_EXPERIMENT_SCHEMA, ANN_EXPERIMENT_STATUS, ANN_LAYER,
    ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_SCHEMA_V1, EXPERIMENT_STATUS_FINISHED,
    LAYER_KIND_RUN_PARAMETERS, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    LocalRegistry, RefUpdate, StoredDescriptor, UnsealedArtifact,
};
use crate::artifact::{media_types, LocalArtifact};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

impl<'reg> UnsealedExperimentState<'reg> {
    /// Consume the unsealed experiment state and commit it as one
    /// immutable artifact. This is the state-level counterpart of the
    /// public `Experiment::commit(self)` lifecycle operation.
    pub fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let config_descriptor = registry.store_empty_config()?;
        let layers = self.artifact_layer_descriptors(registry)?;
        let artifact = UnsealedArtifact::new(
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers,
            None,
            manifest_annotations(),
        );
        let sealed_artifact = registry.seal_artifact(artifact)?;
        let ref_update = registry.publish_manifest_ref(&self.image_name, &sealed_artifact)?;
        if let RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } = ref_update
        {
            crate::bail!(
                "Local registry ref {} already points to {existing_manifest_digest}; \
                 experiment manifest {incoming_manifest_digest} was not published",
                self.image_name
            );
        }

        Ok(LocalArtifact::from_parts(
            registry,
            self.image_name,
            sealed_artifact.digest().clone(),
        ))
    }

    /// OCI layer descriptors that make up the Experiment artifact.
    ///
    /// Record descriptors are already stored when the corresponding
    /// `log_*` call returns. Aggregate descriptors are produced during
    /// commit from the in-memory run table data.
    fn artifact_layer_descriptors(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<Vec<StoredDescriptor<'reg>>> {
        let mut descriptors = self.record_descriptors();
        descriptors.extend(self.aggregate_descriptors(registry)?);
        Ok(descriptors)
    }

    /// Record layers: experiment space first, then each run's space.
    /// The payload bytes were already written when each record was
    /// logged.
    fn record_descriptors(&self) -> Vec<StoredDescriptor<'reg>> {
        let run_records = self.runs.values().flat_map(|run| run.records.iter());
        self.records
            .iter()
            .chain(run_records)
            .map(|record| record.descriptor.clone())
            .collect()
    }

    fn aggregate_descriptors(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<Vec<StoredDescriptor<'reg>>> {
        Ok(vec![store_aggregate_json_layer(
            registry,
            RUN_PARAMETERS_MEDIA_TYPE,
            LAYER_KIND_RUN_PARAMETERS,
            &RunParameterTable::from_runs(self.runs.values())?,
        )?])
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

fn manifest_annotations() -> HashMap<String, String> {
    HashMap::from([
        (
            ANN_ARTIFACT_KIND.to_string(),
            ARTIFACT_KIND_EXPERIMENT.to_string(),
        ),
        (
            ANN_EXPERIMENT_SCHEMA.to_string(),
            EXPERIMENT_SCHEMA_V1.to_string(),
        ),
        (
            ANN_EXPERIMENT_STATUS.to_string(),
            EXPERIMENT_STATUS_FINISHED.to_string(),
        ),
    ])
}
