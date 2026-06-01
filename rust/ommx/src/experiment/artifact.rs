//! Mapping an unsealed Experiment state to an immutable OMMX Artifact.

use super::config::{ExperimentConfig, ExperimentConfigRun, ExperimentConfigSolve, LayerRef};
use super::parameter::RunParameterTable;
use super::UnsealedExperimentState;
use super::{EXPERIMENT_CONFIG_MEDIA_TYPE, RUN_PARAMETERS_MEDIA_TYPE};
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
    pub fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let image_name = self.image_name.clone();
        self.commit_as(
            registry,
            image_name,
            super::EXPERIMENT_STATUS_FINISHED,
            HashMap::new(),
        )
    }

    /// Consume the unsealed experiment state and publish a failed
    /// recovery artifact under a reserved local ref.
    pub(super) fn commit_failed_recovery(
        self,
        registry: &'reg LocalRegistry,
    ) -> Result<LocalArtifact<'reg>> {
        let requested_image_name = self.image_name.clone();
        let recovery_image_name = registry.synthesize_crashed_experiment_image_name()?;
        let mut annotations = HashMap::new();
        annotations.insert(
            super::ANN_EXPERIMENT_STATUS.to_string(),
            super::EXPERIMENT_STATUS_FAILED.to_string(),
        );
        annotations.insert(
            super::ANN_EXPERIMENT_RECOVERY.to_string(),
            "true".to_string(),
        );
        annotations.insert(
            super::ANN_EXPERIMENT_REQUESTED_IMAGE.to_string(),
            requested_image_name.to_string(),
        );

        self.commit_as(
            registry,
            recovery_image_name,
            super::EXPERIMENT_STATUS_FAILED,
            annotations,
        )
    }

    fn commit_as(
        self,
        registry: &'reg LocalRegistry,
        image_name: ImageRef,
        status: &str,
        annotations: HashMap<String, String>,
    ) -> Result<LocalArtifact<'reg>> {
        let run_parameters = self.run_parameter_descriptor(registry)?;
        let mut layers = LayerTable::default();
        let config = self.experiment_config(&mut layers, run_parameters, status)?;
        let config_descriptor = registry.store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )?;
        let artifact = UnsealedArtifact::new(
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers.into_layers(),
            self.subject,
            annotations,
        );
        let sealed_artifact = registry.seal_artifact(artifact)?;
        let ref_update = registry.publish_manifest_ref(&image_name, &sealed_artifact)?;
        if let RefUpdate::Conflicted {
            existing_manifest_digest,
            incoming_manifest_digest,
        } = ref_update
        {
            crate::bail!(
                "Local registry ref {} already points to {existing_manifest_digest}; \
                 experiment manifest {incoming_manifest_digest} was not published",
                image_name
            );
        }

        Ok(LocalArtifact::from_parts(
            registry,
            image_name,
            sealed_artifact.digest().clone(),
        ))
    }

    fn run_parameter_descriptor(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<StoredDescriptor<'reg>> {
        store_aggregate_json_layer(
            registry,
            RUN_PARAMETERS_MEDIA_TYPE,
            &RunParameterTable::from_runs(self.runs.values())?,
        )
    }

    fn experiment_config(
        &self,
        layers: &mut LayerTable<'reg>,
        run_parameters: StoredDescriptor<'reg>,
        status: &str,
    ) -> Result<ExperimentConfig> {
        let attachments = self
            .attachments
            .iter()
            .cloned()
            .map(|descriptor| layers.push(descriptor))
            .collect::<Result<Vec<_>>>()?;

        let mut runs = Vec::new();
        for run in self.runs.values() {
            let attachments = run
                .attachments
                .iter()
                .cloned()
                .map(|descriptor| layers.push(descriptor))
                .collect::<Result<Vec<_>>>()?;
            let trace = run
                .trace
                .clone()
                .map(|descriptor| layers.push(descriptor))
                .transpose()?;
            let mut solves = Vec::new();
            for solve in &run.solves {
                solves.push(ExperimentConfigSolve {
                    solve_id: solve.solve_id,
                    input: layers.push(solve.input.clone())?,
                    output: layers.push(solve.output.clone())?,
                    adapter: solve.adapter.clone(),
                    adapter_options: solve.adapter_options.clone(),
                });
            }
            runs.push(ExperimentConfigRun {
                run_id: run.run_id,
                status: run.status.as_str().to_string(),
                attachments,
                trace,
                solves,
            });
        }

        Ok(ExperimentConfig {
            status: status.to_string(),
            attachments,
            runs,
            run_parameters: layers.push(run_parameters)?,
        })
    }
}

#[derive(Default)]
struct LayerTable<'reg> {
    layers: Vec<StoredDescriptor<'reg>>,
}

impl<'reg> LayerTable<'reg> {
    fn push(&mut self, descriptor: StoredDescriptor<'reg>) -> Result<LayerRef> {
        if self.layers.len() > u32::MAX as usize {
            crate::bail!(
                "Experiment Artifact layer count {} exceeds u32::MAX",
                self.layers.len()
            );
        }
        let index = self.layers.len() as u32;
        self.layers.push(descriptor);
        Ok(LayerRef(index))
    }

    fn into_layers(self) -> Vec<StoredDescriptor<'reg>> {
        self.layers
    }
}

fn store_aggregate_json_layer<'reg>(
    registry: &'reg LocalRegistry,
    media_type: &str,
    value: &impl serde::Serialize,
) -> Result<StoredDescriptor<'reg>> {
    registry.store_json_layer_blob(
        MediaType::Other(media_type.to_string()),
        value,
        Default::default(),
    )
}
