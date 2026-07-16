//! Read-only model reconstructed from a sealed Experiment Artifact.

use super::artifact::ExperimentArtifactView;
use super::attachment::{validate_attachment_storage, AttachmentTable};
use super::config::{ExperimentConfigSampling, ExperimentConfigSolve, LayerRef, LifecycleOutcome};
use super::parameter::{RunParameterCell, RunParameterTable};
use super::{
    read_adapter_diagnostic_payload, AdapterDiagnosticPayload, ExperimentStatus, RunStatus,
    SamplingStatus, SealedExperiment, SolveStatus, Trace, EXPERIMENT_ARTIFACT_MEDIA_TYPE,
    EXPERIMENT_CONFIG_MEDIA_TYPE, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{
    ArtifactManifestRecord, ExperimentManifestRecord, StoredDescriptor,
};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn experiment_lifecycle_reason(
    status: &ExperimentStatus,
    outcome: Option<LifecycleOutcome>,
) -> Result<Option<String>> {
    let reason = outcome.and_then(|outcome| outcome.reason);
    if reason.is_some()
        && !matches!(
            status,
            ExperimentStatus::Failed | ExperimentStatus::Interrupted
        )
    {
        crate::bail!("Experiment status {status} cannot have a lifecycle reason");
    }
    Ok(reason)
}

fn run_lifecycle_reason(
    status: &RunStatus,
    outcome: Option<LifecycleOutcome>,
) -> Result<Option<String>> {
    let reason = outcome.and_then(|outcome| outcome.reason);
    if reason.is_some() && !matches!(status, RunStatus::Failed | RunStatus::Interrupted) {
        crate::bail!("Run status {status} cannot have a lifecycle reason");
    }
    Ok(reason)
}

impl<'reg> SealedExperiment<'reg> {
    /// Reconstruct a finished, failed, or interrupted Experiment Artifact.
    pub fn from_artifact(artifact: LocalArtifact<'reg>) -> Result<Self> {
        Self::from_artifact_with_allowed_statuses(
            artifact,
            &[
                ExperimentStatus::Finished,
                ExperimentStatus::Failed,
                ExperimentStatus::Interrupted,
            ],
        )
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

    /// Reconstruct a sealed Experiment while accepting the given serialized
    /// status set. Listing/checkpoint code uses this to validate non-finished
    /// Experiment artifacts without exposing those states as normal loads.
    pub(crate) fn from_artifact_with_allowed_statuses(
        artifact: LocalArtifact<'reg>,
        allowed_statuses: &[ExperimentStatus],
    ) -> Result<Self> {
        let config = ExperimentArtifactView::new(&artifact).config()?;
        let status = ExperimentStatus::from_config(&config.status)?;
        let reason = experiment_lifecycle_reason(&status, config.outcome)?;
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
            let samplings = decode_samplings(&layers, run.run_id, run.samplings)?;
            let status = RunStatus::from_config(&run.status)
                .with_context(|| format!("Invalid Run {} status", run.run_id))?;
            let reason = run_lifecycle_reason(&status, run.outcome)
                .with_context(|| format!("Invalid Run {} outcome", run.run_id))?;
            if runs
                .insert(
                    run.run_id,
                    SealedRun {
                        run_id: run.run_id,
                        status,
                        reason,
                        attachments,
                        trace,
                        solves,
                        samplings,
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
            reason,
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

    /// Concise caller-provided reason for a failed or interrupted Experiment.
    pub fn lifecycle_reason(&self) -> Option<&str> {
        self.reason.as_deref()
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

    pub fn attachment_instance(&self, name: &str) -> Result<Instance> {
        self.attachments.instance(name)
    }

    pub fn attachment_parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        self.attachments.parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<Solution> {
        self.attachments.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<SampleSet> {
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

/// Build the Local Registry's Experiment listing projection from an artifact.
///
/// This keeps Experiment manifest/config validation owned by the Experiment
/// module while letting the registry store a SQLite read model.
pub(crate) fn experiment_manifest_record_from_artifact(
    artifact: &LocalArtifact<'_>,
) -> Result<Option<ExperimentManifestRecord>> {
    let manifest = artifact.get_manifest()?;
    if manifest.artifact_type().as_ref() != EXPERIMENT_ARTIFACT_MEDIA_TYPE {
        return Ok(None);
    }
    let config_descriptor = manifest.config();
    if config_descriptor.media_type().as_ref() != EXPERIMENT_CONFIG_MEDIA_TYPE {
        crate::bail!(
            "Experiment config media type is {}, expected {}",
            config_descriptor.media_type(),
            EXPERIMENT_CONFIG_MEDIA_TYPE
        );
    }
    let manifest_json = artifact.read_blob_by_digest(artifact.manifest_digest())?;
    let config_json = artifact.get_blob_by_descriptor(&config_descriptor)?;
    SealedExperiment::from_artifact_with_allowed_statuses(
        artifact.clone(),
        &[
            ExperimentStatus::Finished,
            ExperimentStatus::Draft,
            ExperimentStatus::Failed,
            ExperimentStatus::Interrupted,
        ],
    )?;
    let artifact_record = ArtifactManifestRecord::from_image_manifest(
        artifact.manifest_digest().clone(),
        manifest_json,
        manifest.as_image_manifest(),
    )?;
    Ok(Some(ExperimentManifestRecord::from_validated_config(
        artifact_record,
        config_json,
    )?))
}

/// Read-only Run reconstructed from a sealed Experiment config.
#[derive(Debug, Clone)]
pub struct SealedRun<'reg> {
    run_id: u64,
    status: RunStatus,
    reason: Option<String>,
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    trace: Option<StoredDescriptor<'reg>>,
    solves: Vec<Solve<'reg>>,
    samplings: Vec<Sampling<'reg>>,
}

impl<'reg> SealedRun<'reg> {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn status(&self) -> &RunStatus {
        &self.status
    }

    /// Concise caller-provided reason for a failed or interrupted Run.
    pub fn lifecycle_reason(&self) -> Option<&str> {
        self.reason.as_deref()
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

    pub fn attachment_instance(&self, name: &str) -> Result<Instance> {
        self.attachments.instance(name)
    }

    pub fn attachment_parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        self.attachments.parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<Solution> {
        self.attachments.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<SampleSet> {
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

    pub fn samplings(&self) -> &[Sampling<'reg>] {
        &self.samplings
    }
}

#[derive(Debug, Clone)]
pub struct Solve<'reg> {
    solve_id: u64,
    status: SolveStatus,
    input: StoredDescriptor<'reg>,
    output: Option<StoredDescriptor<'reg>>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<StoredDescriptor<'reg>>,
}

impl<'reg> Solve<'reg> {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    pub fn status(&self) -> &SolveStatus {
        &self.status
    }

    /// Internal input descriptor used when sealed state is forked or converted
    /// into a dynamic view. Public solve access returns typed payloads.
    pub(crate) fn input_descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.input
    }

    /// Internal output descriptor used when sealed state is forked or converted
    /// into a dynamic view. Public solve access returns typed payloads.
    pub(crate) fn output_descriptor(&self) -> Option<&StoredDescriptor<'reg>> {
        self.output.as_ref()
    }

    pub(crate) fn diagnostic_descriptor(&self) -> Option<&StoredDescriptor<'reg>> {
        self.diagnostics.as_ref()
    }

    /// Decode the adapter diagnostics payload recorded for this solve.
    pub fn diagnostic_payload(&self) -> Result<Option<AdapterDiagnosticPayload>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let (_, payload) = read_adapter_diagnostic_payload("Solve", self.solve_id, descriptor)?;
        Ok(Some(payload))
    }

    /// Raw MessagePack bytes of the adapter diagnostics payload.
    pub fn diagnostic_blob(&self) -> Result<Option<Vec<u8>>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let (bytes, _) = read_adapter_diagnostic_payload("Solve", self.solve_id, descriptor)?;
        Ok(Some(bytes))
    }

    pub fn input_instance(&self) -> Result<Instance> {
        self.input.registry().get_instance_layer(&self.input)
    }

    /// Decode the Solution returned by this Solve.
    pub fn output_solution(&self) -> Result<Option<Solution>> {
        let Some(output) = &self.output else {
            return Ok(None);
        };
        Ok(Some(output.registry().get_solution_layer(output)?))
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn adapter_options(&self) -> &str {
        &self.adapter_options
    }
}

/// Read-only Sampling record reconstructed from a sealed Experiment config.
#[derive(Debug, Clone)]
pub struct Sampling<'reg> {
    sampling_id: u64,
    status: SamplingStatus,
    input: StoredDescriptor<'reg>,
    output: Option<StoredDescriptor<'reg>>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<StoredDescriptor<'reg>>,
}

impl<'reg> Sampling<'reg> {
    pub fn sampling_id(&self) -> u64 {
        self.sampling_id
    }

    pub fn status(&self) -> &SamplingStatus {
        &self.status
    }

    pub(crate) fn input_descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.input
    }

    pub(crate) fn output_descriptor(&self) -> Option<&StoredDescriptor<'reg>> {
        self.output.as_ref()
    }

    pub(crate) fn diagnostic_descriptor(&self) -> Option<&StoredDescriptor<'reg>> {
        self.diagnostics.as_ref()
    }

    pub fn diagnostic_payload(&self) -> Result<Option<AdapterDiagnosticPayload>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let (_, payload) =
            read_adapter_diagnostic_payload("Sampling", self.sampling_id, descriptor)?;
        Ok(Some(payload))
    }

    pub fn diagnostic_blob(&self) -> Result<Option<Vec<u8>>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let (bytes, _) = read_adapter_diagnostic_payload("Sampling", self.sampling_id, descriptor)?;
        Ok(Some(bytes))
    }

    pub fn input_instance(&self) -> Result<Instance> {
        self.input.registry().get_instance_layer(&self.input)
    }

    /// Decode the SampleSet returned by this Sampling.
    pub fn output_sample_set(&self) -> Result<Option<SampleSet>> {
        let Some(output) = &self.output else {
            return Ok(None);
        };
        Ok(Some(output.registry().get_sample_set_layer(output)?))
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
        let descriptor = resolve_layer(layers, *layer_ref)
            .with_context(|| {
                format!(
                    "Failed to resolve {attachment_context} attachment `{name}` LayerRef {}",
                    layer_ref.0
                )
            })?
            .clone();
        validate_attachment_storage(&descriptor).with_context(|| {
            format!("Invalid {attachment_context} attachment `{name}` storage format")
        })?;
        Ok(descriptor)
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
        crate::artifact::media_types::instance_payload_version(input.media_type())
            .with_context(|| format!("Invalid Run {run_id} Solve {} input", solve.solve_id))?;
        let status = SolveStatus::from_config(&solve.status)
            .with_context(|| format!("Invalid Run {run_id} Solve {} status", solve.solve_id))?;
        if status == SolveStatus::Finished && solve.output.is_none() {
            crate::bail!(
                "Run {run_id} Solve {} is finished but has no output",
                solve.solve_id
            );
        }
        if status != SolveStatus::Finished && solve.output.is_some() {
            crate::bail!(
                "Run {run_id} Solve {} has status {status} but has an output",
                solve.solve_id
            );
        }
        let output = solve
            .output
            .map(|layer_ref| {
                let descriptor = resolve_layer(layers, layer_ref)
                    .with_context(|| {
                        format!(
                            "Failed to resolve Run {run_id} Solve {} output LayerRef {}",
                            solve.solve_id, layer_ref.0
                        )
                    })?
                    .clone();
                anyhow::ensure!(
                    media_types::is_solution_payload_media_type(descriptor.media_type()),
                    "Invalid Run {run_id} Solve {} output media type: {}, expected an OMMX Solution payload",
                    solve.solve_id,
                    descriptor.media_type(),
                );
                Ok::<_, anyhow::Error>(descriptor)
            })
            .transpose()?;
        decoded.push(Solve {
            solve_id: solve.solve_id,
            status,
            input,
            output,
            adapter: solve.adapter,
            adapter_options: solve.adapter_options,
            diagnostics: solve
                .diagnostics
                .map(|layer_ref| {
                    let descriptor = resolve_layer(layers, layer_ref)
                        .with_context(|| {
                            format!(
                                "Failed to resolve Run {run_id} Solve {} diagnostic LayerRef {}",
                                solve.solve_id, layer_ref.0
                            )
                        })?
                        .clone();
                    validate_layer_media_type(&descriptor, &media_types::diagnostic_msgpack())
                        .with_context(|| {
                            format!("Invalid Run {run_id} Solve {} diagnostic", solve.solve_id)
                        })?;
                    let bytes = descriptor.registry().get_blob(&descriptor)?;
                    AdapterDiagnosticPayload::new(bytes).with_context(|| {
                        format!(
                            "Invalid Run {run_id} Solve {} diagnostic payload",
                            solve.solve_id
                        )
                    })?;
                    Ok::<StoredDescriptor<'reg>, anyhow::Error>(descriptor)
                })
                .transpose()?,
        });
    }
    decoded.sort_by_key(Solve::solve_id);
    Ok(decoded)
}

fn decode_samplings<'reg>(
    layers: &[StoredDescriptor<'reg>],
    run_id: u64,
    samplings: Vec<ExperimentConfigSampling>,
) -> Result<Vec<Sampling<'reg>>> {
    let mut decoded = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for sampling in samplings {
        if !seen.insert(sampling.sampling_id) {
            crate::bail!(
                "Run {run_id} contains duplicate Sampling {}",
                sampling.sampling_id
            );
        }
        let input = resolve_layer(layers, sampling.input)
            .with_context(|| {
                format!(
                    "Failed to resolve Run {run_id} Sampling {} input LayerRef {}",
                    sampling.sampling_id, sampling.input.0
                )
            })?
            .clone();
        media_types::instance_payload_version(input.media_type()).with_context(|| {
            format!(
                "Invalid Run {run_id} Sampling {} input",
                sampling.sampling_id
            )
        })?;
        let status = SamplingStatus::from_config(&sampling.status).with_context(|| {
            format!(
                "Invalid Run {run_id} Sampling {} status",
                sampling.sampling_id
            )
        })?;
        if status == SamplingStatus::Finished && sampling.output.is_none() {
            crate::bail!(
                "Run {run_id} Sampling {} is finished but has no output",
                sampling.sampling_id
            );
        }
        if status != SamplingStatus::Finished && sampling.output.is_some() {
            crate::bail!(
                "Run {run_id} Sampling {} has status {status} but has an output",
                sampling.sampling_id
            );
        }
        let output = sampling
            .output
            .map(|layer_ref| {
                let descriptor = resolve_layer(layers, layer_ref)
                    .with_context(|| {
                        format!(
                            "Failed to resolve Run {run_id} Sampling {} output LayerRef {}",
                            sampling.sampling_id, layer_ref.0
                        )
                    })?
                    .clone();
                anyhow::ensure!(
                    media_types::is_sample_set_payload_media_type(descriptor.media_type()),
                    "Invalid Run {run_id} Sampling {} output media type: {}, expected an OMMX SampleSet payload",
                    sampling.sampling_id,
                    descriptor.media_type(),
                );
                Ok::<_, anyhow::Error>(descriptor)
            })
            .transpose()?;
        decoded.push(Sampling {
            sampling_id: sampling.sampling_id,
            status,
            input,
            output,
            adapter: sampling.adapter,
            adapter_options: sampling.adapter_options,
            diagnostics: sampling
                .diagnostics
                .map(|layer_ref| {
                    let descriptor = resolve_layer(layers, layer_ref)
                        .with_context(|| {
                            format!(
                                "Failed to resolve Run {run_id} Sampling {} diagnostic LayerRef {}",
                                sampling.sampling_id, layer_ref.0
                            )
                        })?
                        .clone();
                    validate_layer_media_type(&descriptor, &media_types::diagnostic_msgpack())
                        .with_context(|| {
                            format!(
                                "Invalid Run {run_id} Sampling {} diagnostic",
                                sampling.sampling_id
                            )
                        })?;
                    let bytes = descriptor.registry().get_blob(&descriptor)?;
                    AdapterDiagnosticPayload::new(bytes).with_context(|| {
                        format!(
                            "Invalid Run {run_id} Sampling {} diagnostic payload",
                            sampling.sampling_id
                        )
                    })?;
                    Ok::<StoredDescriptor<'reg>, anyhow::Error>(descriptor)
                })
                .transpose()?,
        });
    }
    decoded.sort_by_key(Sampling::sampling_id);
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
    RunParameterTable::from_msgpack_bytes(&bytes)
        .context("Failed to decode run-parameter table MessagePack")
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
