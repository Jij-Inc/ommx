//! Mapping an unsealed Experiment state to an immutable OMMX Artifact.

use super::config::{ExperimentConfig, ExperimentConfigRun, ExperimentConfigSolve, LayerRef};
use super::parameter::RunParameterTable;
use super::UnsealedExperimentState;
use super::{
    ANN_LAYER, EXPERIMENT_CONFIG_MEDIA_TYPE, LAYER_KIND_RUN_PARAMETERS, RUN_PARAMETERS_MEDIA_TYPE,
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
        let run_parameters = self.run_parameter_descriptor(registry)?;
        let mut layers = LayerTable::default();
        let config = self.experiment_config(&mut layers, run_parameters)?;
        if let Some(trace_layer) = self.trace_layer {
            layers.push(trace_layer)?;
        }
        let config_descriptor = registry.store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )?;
        let artifact = UnsealedArtifact::new(
            MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers.into_layers(),
            self.subject,
            HashMap::new(),
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

    fn run_parameter_descriptor(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<StoredDescriptor<'reg>> {
        store_aggregate_json_layer(
            registry,
            RUN_PARAMETERS_MEDIA_TYPE,
            LAYER_KIND_RUN_PARAMETERS,
            &RunParameterTable::from_runs(self.runs.values())?,
        )
    }

    fn experiment_config(
        &self,
        layers: &mut LayerTable<'reg>,
        run_parameters: StoredDescriptor<'reg>,
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
                attachments,
                solves,
            });
        }

        Ok(ExperimentConfig {
            status: super::EXPERIMENT_STATUS_FINISHED.to_string(),
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
    layer_kind: &str,
    value: &impl serde::Serialize,
) -> Result<StoredDescriptor<'reg>> {
    let mut annotations = HashMap::new();
    annotations.insert(ANN_LAYER.to_string(), layer_kind.to_string());
    registry.store_json_layer_blob(MediaType::Other(media_type.to_string()), value, annotations)
}
