//! Mapping an unsealed Experiment state to an immutable OMMX Artifact.

use super::config::{ExperimentConfig, ExperimentConfigRun, ExperimentConfigSolve, LayerRef};
use super::parameter::RunParameterTable;
use super::{experiment_manifest_record_from_artifact, UnsealedExperimentState};
use super::{
    EXPERIMENT_ARTIFACT_MEDIA_TYPE, EXPERIMENT_CONFIG_MEDIA_TYPE, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    LocalRegistry, RefUpdate, StoredDescriptor, UnsealedArtifact,
};
use crate::artifact::{ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::MediaType;

/// Persistence boundary for the Experiment Artifact layout.
///
/// This view owns the knowledge that an Experiment Artifact is identified by
/// [`EXPERIMENT_ARTIFACT_MEDIA_TYPE`] and that its OCI config blob contains an
/// [`ExperimentConfig`] with [`EXPERIMENT_CONFIG_MEDIA_TYPE`]. The config type
/// itself remains a serialized schema, and sealed / dynamic models only ask
/// this boundary to decode that schema from an artifact.
pub struct ExperimentArtifactView<'a, 'reg> {
    artifact: &'a LocalArtifact<'reg>,
}

impl<'a, 'reg> ExperimentArtifactView<'a, 'reg> {
    pub fn new(artifact: &'a LocalArtifact<'reg>) -> Self {
        Self { artifact }
    }

    pub fn config(&self) -> Result<ExperimentConfig> {
        let manifest = self.artifact.get_manifest()?;
        if manifest.artifact_type() != &MediaType::Other(EXPERIMENT_ARTIFACT_MEDIA_TYPE.to_string())
        {
            crate::bail!(
                "Experiment artifact type is {}, expected {}",
                manifest.artifact_type(),
                EXPERIMENT_ARTIFACT_MEDIA_TYPE
            );
        }
        let config = self.artifact.stored_config()?;
        if config.media_type() != &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()) {
            crate::bail!(
                "Experiment config media type is {}, expected {}",
                config.media_type(),
                EXPERIMENT_CONFIG_MEDIA_TYPE
            );
        }
        let bytes = self.artifact.get_blob(&config)?;
        serde_json::from_slice::<ExperimentConfig>(&bytes)
            .context("Failed to decode Experiment config")
    }
}

impl<'reg> UnsealedExperimentState<'reg> {
    /// Consume the unsealed experiment state and commit it as one
    /// immutable artifact. This is the state-level counterpart of the
    /// public `Experiment::commit(self)` lifecycle operation.
    pub fn commit(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        let image_name = self.image_name.clone();
        let artifact = self.publish_as(
            registry,
            image_name.clone(),
            super::EXPERIMENT_STATUS_FINISHED,
            None,
            RefPublishMode::Publish,
        )?;
        let checkpoint_image_name = registry.experiment_checkpoint_image_name(&image_name)?;
        if let Err(error) = registry.delete_manifest_ref(&checkpoint_image_name) {
            tracing::warn!(
                error = %error,
                checkpoint_image_name = %checkpoint_image_name,
                "Failed to remove Experiment checkpoint ref after commit"
            );
        }
        Ok(artifact)
    }

    /// Consume the unsealed experiment state and publish a checkpoint
    /// manifest under a reserved local ref.
    pub fn commit_checkpoint(
        self,
        registry: &'reg LocalRegistry,
        status: &'static str,
    ) -> Result<LocalArtifact<'reg>> {
        let requested_image_name = self.image_name.clone();
        let checkpoint_image_name =
            registry.experiment_checkpoint_image_name(&requested_image_name)?;

        self.publish_as(
            registry,
            checkpoint_image_name,
            status,
            Some(&requested_image_name),
            RefPublishMode::Replace,
        )
    }

    /// Publish or update the rolling autosave checkpoint for this
    /// unsealed Experiment state.
    pub fn autosave_checkpoint(
        &mut self,
        registry: &'reg LocalRegistry,
    ) -> Result<LocalArtifact<'reg>> {
        let image_name = registry.experiment_checkpoint_image_name(&self.image_name)?;
        let artifact = self.publish_as(
            registry,
            image_name,
            super::EXPERIMENT_STATUS_DRAFT,
            Some(&self.image_name),
            RefPublishMode::Replace,
        )?;
        Ok(artifact)
    }

    fn publish_as(
        &self,
        registry: &'reg LocalRegistry,
        image_name: ImageRef,
        status: &str,
        requested_image_name: Option<&ImageRef>,
        publish_mode: RefPublishMode,
    ) -> Result<LocalArtifact<'reg>> {
        let run_parameters = self.run_parameter_descriptor(registry)?;
        let mut layers = LayerTable::default();
        let config =
            self.experiment_config(&mut layers, run_parameters, status, requested_image_name)?;
        let config_descriptor = registry.store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )?;
        let artifact = UnsealedArtifact::new(
            MediaType::Other(EXPERIMENT_ARTIFACT_MEDIA_TYPE.to_string()),
            config_descriptor,
            layers.into_layers(),
            self.subject.clone(),
            self.annotations.clone(),
        );
        let sealed_artifact = registry.seal_artifact(artifact)?;
        let local_artifact = LocalArtifact::from_parts(
            registry,
            image_name.clone(),
            sealed_artifact.digest().clone(),
        );
        let experiment_record = experiment_manifest_record_from_artifact(&local_artifact)?
            .context("Committed Experiment artifact should be indexable as an Experiment")?;
        let ref_update = match publish_mode {
            RefPublishMode::Publish => registry.publish_experiment_manifest_ref(
                &image_name,
                &sealed_artifact,
                &experiment_record,
            ),
            RefPublishMode::Replace => registry.replace_experiment_manifest_ref(
                &image_name,
                &sealed_artifact,
                &experiment_record,
            ),
        }?;
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
        requested_image_name: Option<&ImageRef>,
    ) -> Result<ExperimentConfig> {
        let attachments = self
            .attachments
            .try_map(|_, descriptor| layers.push(descriptor.clone()))?;

        let mut runs = Vec::new();
        for run in self.runs.values() {
            let attachments = run
                .attachments
                .try_map(|_, descriptor| layers.push(descriptor.clone()))?;
            let trace = run
                .trace
                .clone()
                .map(|descriptor| layers.push(descriptor))
                .transpose()?;
            let mut solves = Vec::new();
            for solve in &run.solves {
                solves.push(ExperimentConfigSolve {
                    solve_id: solve.solve_id,
                    status: solve.status.as_str().to_string(),
                    input: layers.push(solve.input.clone())?,
                    output: solve
                        .output
                        .clone()
                        .map(|descriptor| layers.push(descriptor))
                        .transpose()?,
                    adapter: solve.adapter.clone(),
                    adapter_options: solve.adapter_options.clone(),
                    diagnostics: solve
                        .diagnostics
                        .clone()
                        .map(|descriptor| layers.push(descriptor))
                        .transpose()?,
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
            requested_image_name: requested_image_name.map(ToString::to_string),
            attachments,
            runs,
            run_parameters: layers.push(run_parameters)?,
        })
    }
}

#[derive(Clone, Copy)]
enum RefPublishMode {
    Publish,
    Replace,
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
    registry.store_json_layer_blob(MediaType::from(media_type), value, Default::default())
}
