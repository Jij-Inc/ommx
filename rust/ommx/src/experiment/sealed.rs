//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::attachment::attachment_name;
use super::config::{ExperimentConfig, ExperimentConfigSolve, LayerRef};
use super::parameter::{RunParameterCell, RunParameterTable};
use super::{
    RunStatus, SealedExperiment, EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_STATUS_FAILED,
    EXPERIMENT_STATUS_FINISHED, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::StoredDescriptor;
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::BTreeMap;

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        Self::from_artifact_with_allowed_statuses(artifact, &[EXPERIMENT_STATUS_FINISHED])
    }

    /// Reconstruct a failed recovery Experiment from a committed recovery Artifact.
    pub fn from_recovery_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        Self::from_artifact_with_allowed_statuses(artifact, &[EXPERIMENT_STATUS_FAILED])
    }

    fn from_artifact_with_allowed_statuses(
        artifact: LocalArtifact<'reg>,
        allowed_statuses: &[&str],
    ) -> Result<Self> {
        let config = load_experiment_config(&artifact, allowed_statuses)?;
        let status = config.status.clone();
        let layers = artifact.layers()?;

        let attachments = decode_attachments(&layers, config.attachments, "experiment")?;
        let mut runs = BTreeMap::new();
        for run in config.runs {
            let attachments =
                decode_attachments(&layers, run.attachments, &format!("run {}", run.run_id))?;
            let trace = decode_trace(&layers, run.trace, run.run_id)?;
            let solves = decode_solves(&layers, run.run_id, run.solves)?;
            let status = RunStatus::from_config(&run.status)
                .with_context(|| format!("Invalid Run {} status", run.run_id))?;
            if runs
                .insert(
                    run.run_id,
                    SealedRun {
                        run_id: run.run_id,
                        status,
                        failure_reason: run.failure_reason,
                        attachments,
                        trace,
                        solves,
                    },
                )
                .is_some()
            {
                crate::bail!("Experiment config contains duplicate Run {}", run.run_id);
            }
        }
        let run_parameters = load_run_parameters(&artifact, &layers, config.run_parameters)?;
        validate_run_parameters_reference_config_runs(&run_parameters, &runs)?;

        Ok(Self {
            status,
            artifact,
            attachments,
            runs,
            run_parameters,
        })
    }

    pub fn image_name(&self) -> &ImageRef {
        self.artifact.image_name()
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn experiment_attachments(&self) -> &[StoredDescriptor<'reg>] {
        &self.attachments
    }

    pub fn runs(&self) -> impl Iterator<Item = &SealedRun<'reg>> {
        self.runs.values()
    }

    pub fn run(&self, run_id: u64) -> Option<&SealedRun<'reg>> {
        self.runs.get(&run_id)
    }

    pub fn run_parameter_cells(&self) -> Vec<RunParameterCell> {
        self.run_parameters.cells()
    }
}

/// Read-only Run reconstructed from a sealed Experiment config.
#[derive(Debug, Clone)]
pub struct SealedRun<'reg> {
    run_id: u64,
    status: RunStatus,
    failure_reason: Option<String>,
    attachments: Vec<StoredDescriptor<'reg>>,
    trace: Option<StoredDescriptor<'reg>>,
    solves: Vec<Solve<'reg>>,
}

impl<'reg> SealedRun<'reg> {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn status(&self) -> &RunStatus {
        &self.status
    }

    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    pub fn attachments(&self) -> &[StoredDescriptor<'reg>] {
        &self.attachments
    }

    pub fn trace(&self) -> Option<&StoredDescriptor<'reg>> {
        self.trace.as_ref()
    }

    pub fn solves(&self) -> &[Solve<'reg>] {
        &self.solves
    }
}

#[derive(Debug, Clone)]
pub struct Solve<'reg> {
    solve_id: u64,
    input: StoredDescriptor<'reg>,
    output: StoredDescriptor<'reg>,
    adapter: String,
    adapter_options: String,
}

impl<'reg> Solve<'reg> {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    pub fn input(&self) -> &StoredDescriptor<'reg> {
        &self.input
    }

    pub fn output(&self) -> &StoredDescriptor<'reg> {
        &self.output
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn adapter_options(&self) -> &str {
        &self.adapter_options
    }
}

fn load_experiment_config(
    artifact: &LocalArtifact<'_>,
    allowed_statuses: &[&str],
) -> Result<ExperimentConfig> {
    let config = artifact.stored_config()?;
    if config.media_type() != &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Experiment config media type is {}, expected {}",
            config.media_type(),
            EXPERIMENT_CONFIG_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(&config)?;
    let config = serde_json::from_slice::<ExperimentConfig>(&bytes)
        .context("Failed to decode Experiment config")?;
    if !allowed_statuses
        .iter()
        .any(|status| config.status == *status)
    {
        let expected = allowed_statuses.join(" or ");
        crate::bail!(
            "Experiment config status is {}, expected {}",
            config.status,
            expected
        );
    }
    Ok(config)
}

fn decode_attachments<'reg>(
    layers: &[StoredDescriptor<'reg>],
    attachments: Vec<LayerRef>,
    attachment_context: &str,
) -> Result<Vec<StoredDescriptor<'reg>>> {
    let mut decoded = Vec::new();
    for layer_ref in attachments {
        let descriptor = resolve_layer(layers, layer_ref)
            .with_context(|| {
                format!(
                    "Failed to resolve {attachment_context} attachment LayerRef {}",
                    layer_ref.0
                )
            })?
            .clone();
        if attachment_name(&descriptor).is_none() {
            crate::bail!("Attachment descriptor in {attachment_context} is missing `org.ommx.attachment.name`");
        }
        decoded.push(descriptor);
    }
    Ok(decoded)
}

fn decode_trace<'reg>(
    layers: &[StoredDescriptor<'reg>],
    trace: Option<LayerRef>,
    run_id: u64,
) -> Result<Option<StoredDescriptor<'reg>>> {
    let Some(layer_ref) = trace else {
        return Ok(None);
    };
    let descriptor = resolve_layer(layers, layer_ref)
        .with_context(|| format!("Failed to resolve Run {run_id} trace ref {}", layer_ref.0))?
        .clone();
    validate_layer_media_type(&descriptor, &media_types::trace_otlp_protobuf())
        .with_context(|| format!("Invalid Run {run_id} trace"))?;
    Ok(Some(descriptor))
}

fn decode_solves<'reg>(
    layers: &[StoredDescriptor<'reg>],
    run_id: u64,
    solves: Vec<ExperimentConfigSolve>,
) -> Result<Vec<Solve<'reg>>> {
    let mut decoded = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for solve in solves {
        if !seen.insert(solve.solve_id) {
            crate::bail!("Run {run_id} contains duplicate Solve {}", solve.solve_id);
        }
        let input = resolve_layer(layers, solve.input)
            .with_context(|| {
                format!(
                    "Failed to resolve Run {run_id} Solve {} input LayerRef {}",
                    solve.solve_id, solve.input.0
                )
            })?
            .clone();
        validate_layer_media_type(&input, &crate::artifact::media_types::v1_instance())
            .with_context(|| format!("Invalid Run {run_id} Solve {} input", solve.solve_id))?;
        let output = resolve_layer(layers, solve.output)
            .with_context(|| {
                format!(
                    "Failed to resolve Run {run_id} Solve {} output LayerRef {}",
                    solve.solve_id, solve.output.0
                )
            })?
            .clone();
        validate_layer_media_type(&output, &crate::artifact::media_types::v1_solution())
            .with_context(|| format!("Invalid Run {run_id} Solve {} output", solve.solve_id))?;
        decoded.push(Solve {
            solve_id: solve.solve_id,
            input,
            output,
            adapter: solve.adapter,
            adapter_options: solve.adapter_options,
        });
    }
    Ok(decoded)
}

fn load_run_parameters(
    artifact: &LocalArtifact<'_>,
    layers: &[StoredDescriptor<'_>],
    layer_ref: LayerRef,
) -> Result<RunParameterTable> {
    let descriptor = resolve_layer(layers, layer_ref).with_context(|| {
        format!(
            "Failed to resolve run-parameter table LayerRef {}",
            layer_ref.0
        )
    })?;
    if descriptor.media_type() != &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()) {
        crate::bail!(
            "Run-parameter descriptor media type is {}, expected {}",
            descriptor.media_type(),
            RUN_PARAMETERS_MEDIA_TYPE
        );
    }
    let bytes = artifact.get_blob(descriptor)?;
    serde_json::from_slice::<RunParameterTable>(&bytes)
        .context("Failed to decode run-parameter table JSON")
}

fn resolve_layer<'a, 'reg>(
    layers: &'a [StoredDescriptor<'reg>],
    layer_ref: LayerRef,
) -> Result<&'a StoredDescriptor<'reg>> {
    layers.get(layer_ref.0 as usize).ok_or_else(|| {
        anyhow::anyhow!(
            "LayerRef {} is out of bounds for {} manifest layer(s)",
            layer_ref.0,
            layers.len()
        )
    })
}

fn validate_layer_media_type(descriptor: &Descriptor, expected: &MediaType) -> Result<()> {
    if descriptor.media_type() != expected {
        crate::bail!(
            "media type is {}, expected {}",
            descriptor.media_type(),
            expected
        );
    }
    Ok(())
}

fn validate_run_parameters_reference_config_runs(
    run_parameters: &RunParameterTable,
    runs: &BTreeMap<u64, SealedRun<'_>>,
) -> Result<()> {
    for cell in run_parameters.cells() {
        if !runs.contains_key(&cell.run_id) {
            crate::bail!(
                "Run-parameter table references Run {}, but Experiment config does not contain it",
                cell.run_id
            );
        }
    }
    Ok(())
}
