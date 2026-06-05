//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::artifact::ExperimentArtifactView;
use super::attachment::AttachmentTable;
use super::config::{ExperimentConfigSolve, LayerRef};
use super::parameter::{RunParameterCell, RunParameterTable};
use super::{ExperimentStatus, RunStatus, SealedExperiment, Trace, RUN_PARAMETERS_MEDIA_TYPE};
use crate::artifact::local_registry::StoredDescriptor;
use crate::artifact::{
    media_types, ImageRef, InstanceAnnotations, LocalArtifact, ParametricInstanceAnnotations,
    SampleSetAnnotations, SolutionAnnotations,
};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a sealed Experiment from a committed Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        Self::from_artifact_with_allowed_statuses(artifact, &[ExperimentStatus::Finished])
    }

    /// Reconstruct a checkpoint Experiment from a committed Artifact.
    pub(crate) fn from_checkpoint_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        Self::from_artifact_with_allowed_statuses(
            artifact,
            &[
                ExperimentStatus::Draft,
                ExperimentStatus::Failed,
                ExperimentStatus::Interrupted,
            ],
        )
    }

    fn from_artifact_with_allowed_statuses(
        artifact: LocalArtifact<'reg>,
        allowed_statuses: &[ExperimentStatus],
    ) -> Result<Self> {
        let config = ExperimentArtifactView::new(&artifact).config()?;
        let status = ExperimentStatus::from_config(&config.status)?;
        if !allowed_statuses.contains(&status) {
            let expected = allowed_statuses
                .iter()
                .map(ExperimentStatus::as_str)
                .collect::<Vec<_>>()
                .join(" or ");
            crate::bail!(
                "Experiment config status is {}, expected {}",
                status,
                expected
            );
        }
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

    pub fn status(&self) -> &ExperimentStatus {
        &self.status
    }

    /// Internal descriptor table used when sealed state is forked or converted
    /// into a dynamic view. Public attachment access remains name-based.
    pub(crate) fn attachment_table(&self) -> &AttachmentTable<StoredDescriptor<'reg>> {
        &self.attachments
    }

    pub fn attachment_names(&self) -> impl Iterator<Item = &str> {
        self.attachments.names()
    }

    pub fn contains_attachment(&self, name: &str) -> bool {
        self.attachments.contains_key(name)
    }

    pub fn attachment_media_type(&self, name: &str) -> Result<MediaType> {
        self.attachments.media_type(name)
    }

    pub fn attachment_blob(&self, name: &str) -> Result<Vec<u8>> {
        self.attachments.blob(name)
    }

    pub fn attachment_instance(&self, name: &str) -> Result<(Instance, InstanceAnnotations)> {
        self.attachments.instance(name)
    }

    pub fn attachment_parametric_instance(
        &self,
        name: &str,
    ) -> Result<(ParametricInstance, ParametricInstanceAnnotations)> {
        self.attachments.parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<(Solution, SolutionAnnotations)> {
        self.attachments.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<(SampleSet, SampleSetAnnotations)> {
        self.attachments.sample_set(name)
    }

    pub fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<PathBuf> {
        self.attachments.write_attachment(name, path, overwrite)
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
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
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

    /// Internal descriptor table used when sealed state is forked or converted
    /// into a dynamic view. Public attachment access remains name-based.
    pub(crate) fn attachment_table(&self) -> &AttachmentTable<StoredDescriptor<'reg>> {
        &self.attachments
    }

    pub fn attachment_names(&self) -> impl Iterator<Item = &str> {
        self.attachments.names()
    }

    pub fn contains_attachment(&self, name: &str) -> bool {
        self.attachments.contains_key(name)
    }

    pub fn attachment_media_type(&self, name: &str) -> Result<MediaType> {
        self.attachments.media_type(name)
    }

    pub fn attachment_blob(&self, name: &str) -> Result<Vec<u8>> {
        self.attachments.blob(name)
    }

    pub fn attachment_instance(&self, name: &str) -> Result<(Instance, InstanceAnnotations)> {
        self.attachments.instance(name)
    }

    pub fn attachment_parametric_instance(
        &self,
        name: &str,
    ) -> Result<(ParametricInstance, ParametricInstanceAnnotations)> {
        self.attachments.parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<(Solution, SolutionAnnotations)> {
        self.attachments.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<(SampleSet, SampleSetAnnotations)> {
        self.attachments.sample_set(name)
    }

    pub fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<PathBuf> {
        self.attachments.write_attachment(name, path, overwrite)
    }

    /// Internal trace descriptor used when sealed state is forked or converted
    /// into a dynamic view. Public trace access returns the opaque payload.
    pub(crate) fn trace_descriptor(&self) -> Option<&StoredDescriptor<'reg>> {
        self.trace.as_ref()
    }

    pub fn trace(&self) -> Result<Option<Trace>> {
        let Some(descriptor) = &self.trace else {
            return Ok(None);
        };
        let bytes = descriptor.registry().get_blob(descriptor)?;
        Ok(Some(Trace::from_bytes(bytes)))
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
    diagnostics: Vec<StoredDescriptor<'reg>>,
}

impl<'reg> Solve<'reg> {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    /// Internal input descriptor used when sealed state is forked or converted
    /// into a dynamic view. Public solve access returns typed payloads.
    pub(crate) fn input_descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.input
    }

    /// Internal output descriptor used when sealed state is forked or converted
    /// into a dynamic view. Public solve access returns typed payloads.
    pub(crate) fn output_descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.output
    }

    pub(crate) fn diagnostic_descriptors(&self) -> &[StoredDescriptor<'reg>] {
        &self.diagnostics
    }

    pub fn input_instance(&self) -> Result<(Instance, InstanceAnnotations)> {
        self.input.ensure_media_type(&media_types::v1_instance())?;
        let bytes = self.input.registry().get_blob(&self.input)?;
        Ok((
            Instance::from_bytes(&bytes)?,
            InstanceAnnotations::from_descriptor(&self.input),
        ))
    }

    pub fn output_solution(&self) -> Result<(Solution, SolutionAnnotations)> {
        self.output.ensure_media_type(&media_types::v1_solution())?;
        let bytes = self.output.registry().get_blob(&self.output)?;
        Ok((
            Solution::from_bytes(&bytes)?,
            SolutionAnnotations::from_descriptor(&self.output),
        ))
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn adapter_options(&self) -> &str {
        &self.adapter_options
    }
}

fn decode_attachments<'reg>(
    layers: &[StoredDescriptor<'reg>],
    attachments: AttachmentTable<LayerRef>,
    attachment_context: &str,
) -> Result<AttachmentTable<StoredDescriptor<'reg>>> {
    attachments.try_map(|name, layer_ref| {
        Ok(resolve_layer(layers, *layer_ref)
            .with_context(|| {
                format!(
                    "Failed to resolve {attachment_context} attachment `{name}` LayerRef {}",
                    layer_ref.0
                )
            })?
            .clone())
    })
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
            diagnostics: solve
                .diagnostics
                .into_iter()
                .map(|layer_ref| {
                    let descriptor = resolve_layer(layers, layer_ref)
                        .with_context(|| {
                            format!(
                                "Failed to resolve Run {run_id} Solve {} diagnostic LayerRef {}",
                                solve.solve_id, layer_ref.0
                            )
                        })?
                        .clone();
                    validate_layer_media_type(&descriptor, &media_types::python_pickle())
                        .with_context(|| {
                            format!("Invalid Run {run_id} Solve {} diagnostic", solve.solve_id)
                        })?;
                    Ok(descriptor)
                })
                .collect::<Result<Vec<_>>>()?,
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
