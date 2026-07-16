//! Dynamic-lifetime Experiment / Run handles.
//!
//! [`Experiment`] and [`Run`] use Rust lifetimes to prove that a run
//! cannot outlive its parent experiment and that registry-backed values
//! cannot outlive their [`LocalRegistry`](crate::artifact::local_registry::LocalRegistry).
//! Dynamic runtimes such as Python cannot carry those lifetimes in
//! their object model, so this module provides owned handles that keep
//! the required registry / parent owners alive at runtime.
//!
//! Dynamic state stores plain OCI [`Descriptor`] values internally
//! because it cannot store [`StoredDescriptor`] values whose lifetime is
//! tied to the registry reference inside the same owned handle. This is
//! an internal representation detail: public accessors promote those raw
//! descriptors before decoding typed payloads or writing attachment files.

use super::artifact::ExperimentArtifactView;
use super::config::ExperimentConfig;
use super::logging::AttachmentLoggerStorage;
use super::{
    allocate_next_run_id, next_run_id, read_adapter_diagnostic_payload, AdapterDiagnosticPayload,
    AttachmentTable, AutosaveController, AutosavePolicy, ExperimentLifecycle, ExperimentStatus,
    Name, RunEntry, RunLifecycle, RunParameterCell, RunStatus, SamplingStatus, SealedExperiment,
    SolveStatus, UnsealedExperimentState,
};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::{
    media_types, AsArtifact, ImageRef, LocalArtifact, LocalArtifactDyn, LocalRegistryHandle,
};
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::{ensure, Context, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

mod lifecycle;
mod run;

pub use run::RunDyn;

/// Runtime-owned Experiment handle.
///
/// This is the dynamic-lifetime counterpart of [`super::Experiment`].
/// It stores the registry owner, the unsealed / sealed state, and the
/// count of still-open [`RunDyn`] handles in Rust SDK code so bindings
/// do not need to duplicate these invariants.
///
/// The dynamic state may contain raw [`Descriptor`] values for
/// registry-backed payloads. `ExperimentDyn` also keeps the
/// [`LocalRegistryHandle`] needed to promote those descriptors back to
/// [`StoredDescriptor`] values when accessors return them.
#[derive(Debug)]
pub struct ExperimentDyn {
    registry_handle: LocalRegistryHandle,
    state: Arc<Mutex<ExperimentDynState>>,
    interrupted_reason_on_drop: Mutex<Option<String>>,
}

impl Clone for ExperimentDyn {
    fn clone(&self) -> Self {
        Self {
            registry_handle: self.registry_handle.clone(),
            state: Arc::clone(&self.state),
            // Drop behavior belongs to one handle and must never be inherited
            // by an ordinary clone of the shared Experiment state.
            interrupted_reason_on_drop: Mutex::new(None),
        }
    }
}

#[derive(Debug)]
struct ExperimentDynState {
    lifecycle: ExperimentDynLifecycle,
    registry_handle: LocalRegistryHandle,
    pending_interrupted_checkpoint: Option<String>,
}

#[derive(Debug)]
enum ExperimentDynLifecycle {
    Unsealed {
        state: Option<Box<UnsealedExperimentDynState>>,
        open_runs: usize,
    },
    Sealed(SealedExperimentDynState),
    Checkpoint {
        image_name: ImageRef,
        sealed: SealedExperimentDynState,
    },
    CommitFailed {
        image_name: ImageRef,
        reason: String,
    },
}

#[derive(Debug)]
struct ExperimentCheckpoint {
    artifact: LocalArtifactDyn,
    config: ExperimentConfig,
}

impl ExperimentCheckpoint {
    fn open(
        registry_handle: LocalRegistryHandle,
        requested_image_name: &ImageRef,
        accepted_statuses: &[&str],
    ) -> Result<Self> {
        let checkpoint_image_name = registry_handle
            .registry()
            .experiment_checkpoint_image_name(requested_image_name)?;
        let requested_image_name = requested_image_name.to_string();
        let missing_checkpoint_message = format!(
            "No Experiment checkpoint found for requested image \
             {requested_image_name} at {checkpoint_image_name}"
        );
        let artifact =
            LocalArtifactDyn::open_in_registry_handle(registry_handle, checkpoint_image_name)
                .with_context(|| missing_checkpoint_message)?;
        let checkpoint = Self::from_artifact(artifact)?;
        checkpoint.ensure_requested_image_name(&requested_image_name)?;
        checkpoint.ensure_status(accepted_statuses)?;
        Ok(checkpoint)
    }

    fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let local_artifact = artifact.as_local_artifact();
        let config = ExperimentArtifactView::new(&local_artifact).config()?;
        Ok(Self { artifact, config })
    }

    fn requested_image_name(&self) -> Result<ImageRef> {
        let requested = self.config.requested_image_name.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Experiment checkpoint config is missing requested_image_name")
        })?;
        ImageRef::parse(requested)
            .with_context(|| "Invalid requested_image_name in Experiment checkpoint config")
    }

    fn ensure_requested_image_name(&self, requested_image_name: &str) -> Result<()> {
        let Some(actual_requested_image_name) = self.config.requested_image_name.as_deref() else {
            crate::bail!(
                "Experiment checkpoint {} is missing requested_image_name",
                self.artifact.image_name()
            );
        };
        ensure!(
            actual_requested_image_name == requested_image_name,
            "Experiment checkpoint {} belongs to requested image {actual_requested_image_name}, not {requested_image_name}",
            self.artifact.image_name(),
        );
        Ok(())
    }

    fn ensure_status(&self, accepted_statuses: &[&str]) -> Result<()> {
        ensure!(
            accepted_statuses.contains(&self.config.lifecycle.status().as_str()),
            "Experiment checkpoint {} has status {}",
            self.artifact.image_name(),
            self.config.lifecycle.status()
        );
        Ok(())
    }

    fn into_artifact(self) -> LocalArtifactDyn {
        self.artifact
    }
}

#[derive(Debug)]
struct UnsealedExperimentDynState {
    image_name: ImageRef,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
    attachments: AttachmentTable<Descriptor>,
    runs: BTreeMap<u64, RunEntryDyn>,
    next_run_id: u64,
    autosave: AutosaveController,
}

#[derive(Debug)]
struct RunEntryDyn {
    run_id: u64,
    lifecycle: RunLifecycle,
    attachments: AttachmentTable<Descriptor>,
    trace: Option<Descriptor>,
    solves: Vec<SolveEntryDyn>,
    samplings: Vec<SamplingEntryDyn>,
    parameters: super::parameter::ParameterSet,
}

#[derive(Debug)]
struct SolveEntryDyn {
    solve_id: u64,
    status: SolveStatus,
    input: Descriptor,
    output: Option<Descriptor>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<Descriptor>,
}

#[derive(Debug)]
struct SamplingEntryDyn {
    sampling_id: u64,
    status: SamplingStatus,
    input: Descriptor,
    output: Option<Descriptor>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<Descriptor>,
}

#[derive(Debug, Clone)]
struct SealedExperimentDynState {
    lifecycle: ExperimentLifecycle,
    artifact: LocalArtifactDyn,
    attachments: AttachmentTable<Descriptor>,
    runs: BTreeMap<u64, SealedRunDyn>,
    run_parameters: super::parameter::RunParameterTable,
}

/// Runtime-owned sealed Run view.
///
/// `SealedRunDyn` stores raw attachment descriptors internally because
/// it cannot borrow the registry through a Rust lifetime. Attachment
/// accessors use the stored registry handle to verify and promote them
/// to [`StoredDescriptor`] before exposing them.
#[derive(Debug, Clone)]
pub struct SealedRunDyn {
    registry_handle: LocalRegistryHandle,
    run_id: u64,
    lifecycle: RunLifecycle,
    attachments: AttachmentTable<Descriptor>,
    trace: Option<Descriptor>,
    solves: Vec<SolveDyn>,
    samplings: Vec<SamplingDyn>,
}

/// Runtime-owned Solve view.
///
/// The input and output are stored as raw descriptors in the dynamic
/// state, but public solve accessors never expose those raw values. They
/// re-check the referenced blobs against the associated Local Registry
/// and return decoded payloads.
#[derive(Debug, Clone)]
pub struct SolveDyn {
    registry_handle: LocalRegistryHandle,
    solve_id: u64,
    status: SolveStatus,
    input: Descriptor,
    output: Option<Descriptor>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<Descriptor>,
}

/// Runtime-owned Sampling view.
#[derive(Debug, Clone)]
pub struct SamplingDyn {
    registry_handle: LocalRegistryHandle,
    sampling_id: u64,
    status: SamplingStatus,
    input: Descriptor,
    output: Option<Descriptor>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<Descriptor>,
}

impl SealedRunDyn {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn status(&self) -> &RunStatus {
        self.lifecycle.status()
    }

    /// Complete lifecycle value, including failure metadata where applicable.
    pub fn lifecycle(&self) -> &RunLifecycle {
        &self.lifecycle
    }

    /// Concise caller-provided reason for a failed or interrupted Run.
    pub fn lifecycle_reason(&self) -> Option<&str> {
        self.lifecycle.reason()
    }

    pub fn registry_handle(&self) -> LocalRegistryHandle {
        self.registry_handle.clone()
    }

    fn attachment_table(&self) -> Result<AttachmentTable<StoredDescriptor<'_>>> {
        self.attachments.clone().try_map_owned(|descriptor| {
            self.registry_handle
                .registry()
                .stored_descriptor(descriptor)
        })
    }

    pub fn attachment_names(&self) -> Vec<String> {
        self.attachments.names().map(ToOwned::to_owned).collect()
    }

    pub fn attachment_media_type(&self, name: &str) -> Result<MediaType> {
        self.attachment_table()?.media_type(name)
    }

    pub fn attachment_blob(&self, name: &str) -> Result<Vec<u8>> {
        self.attachment_table()?.blob(name)
    }

    pub fn attachment_instance(&self, name: &str) -> Result<Instance> {
        self.attachment_table()?.instance(name)
    }

    pub fn attachment_parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        self.attachment_table()?.parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<Solution> {
        self.attachment_table()?.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<SampleSet> {
        self.attachment_table()?.sample_set(name)
    }

    pub fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<std::path::PathBuf> {
        self.attachment_table()?
            .write_attachment(name, path, overwrite)
    }

    fn trace_descriptor(&self) -> Result<Option<StoredDescriptor<'_>>> {
        self.trace
            .clone()
            .map(|descriptor| {
                self.registry_handle
                    .registry()
                    .stored_descriptor(descriptor)
            })
            .transpose()
    }

    pub fn trace(&self) -> Result<Option<super::Trace>> {
        let Some(descriptor) = self.trace_descriptor()? else {
            return Ok(None);
        };
        let bytes = self.registry_handle.registry().get_blob(&descriptor)?;
        Ok(Some(super::Trace::from_bytes(bytes)))
    }

    pub fn attachment_count(&self) -> usize {
        self.attachments.len()
    }

    pub fn solves(&self) -> &[SolveDyn] {
        &self.solves
    }

    pub fn samplings(&self) -> &[SamplingDyn] {
        &self.samplings
    }
}

impl SolveDyn {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    pub fn status(&self) -> &SolveStatus {
        &self.status
    }

    fn input_descriptor(&self) -> Result<StoredDescriptor<'_>> {
        self.registry_handle
            .registry()
            .stored_descriptor(self.input.clone())
    }

    pub fn input_instance(&self) -> Result<Instance> {
        let descriptor = self.input_descriptor()?;
        media_types::instance_payload_version(descriptor.media_type())
            .with_context(|| format!("Invalid Solve {} input", self.solve_id))?;
        self.registry_handle
            .registry()
            .get_instance_layer(&descriptor)
    }

    fn output_descriptor(&self) -> Result<Option<StoredDescriptor<'_>>> {
        self.output
            .clone()
            .map(|descriptor| {
                self.registry_handle
                    .registry()
                    .stored_descriptor(descriptor)
            })
            .transpose()
    }

    /// Decode the Solution returned by this Solve.
    pub fn output_solution(&self) -> Result<Option<Solution>> {
        let Some(descriptor) = self.output_descriptor()? else {
            return Ok(None);
        };
        Ok(Some(
            self.registry_handle
                .registry()
                .get_solution_layer(&descriptor)
                .with_context(|| format!("Invalid Solve {} output", self.solve_id))?,
        ))
    }

    /// Raw MessagePack bytes of the adapter diagnostics payload.
    pub fn diagnostic_blob(&self) -> Result<Option<Vec<u8>>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let descriptor = self
            .registry_handle
            .registry()
            .stored_descriptor(descriptor.clone())?;
        let (bytes, _) = read_adapter_diagnostic_payload("Solve", self.solve_id, &descriptor)?;
        Ok(Some(bytes))
    }

    /// Decode the adapter diagnostics payload recorded for this solve.
    pub fn diagnostic_payload(&self) -> Result<Option<AdapterDiagnosticPayload>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let descriptor = self
            .registry_handle
            .registry()
            .stored_descriptor(descriptor.clone())?;
        let (_, payload) = read_adapter_diagnostic_payload("Solve", self.solve_id, &descriptor)?;
        Ok(Some(payload))
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn adapter_options(&self) -> &str {
        &self.adapter_options
    }
}

impl SamplingDyn {
    pub fn sampling_id(&self) -> u64 {
        self.sampling_id
    }

    pub fn status(&self) -> &SamplingStatus {
        &self.status
    }

    fn input_descriptor(&self) -> Result<StoredDescriptor<'_>> {
        self.registry_handle
            .registry()
            .stored_descriptor(self.input.clone())
    }

    pub fn input_instance(&self) -> Result<Instance> {
        let descriptor = self.input_descriptor()?;
        media_types::instance_payload_version(descriptor.media_type())
            .with_context(|| format!("Invalid Sampling {} input", self.sampling_id))?;
        self.registry_handle
            .registry()
            .get_instance_layer(&descriptor)
    }

    fn output_descriptor(&self) -> Result<Option<StoredDescriptor<'_>>> {
        self.output
            .clone()
            .map(|descriptor| {
                self.registry_handle
                    .registry()
                    .stored_descriptor(descriptor)
            })
            .transpose()
    }

    pub fn output_sample_set(&self) -> Result<Option<SampleSet>> {
        let Some(descriptor) = self.output_descriptor()? else {
            return Ok(None);
        };
        Ok(Some(
            self.registry_handle
                .registry()
                .get_sample_set_layer(&descriptor)
                .with_context(|| format!("Invalid Sampling {} output", self.sampling_id))?,
        ))
    }

    pub fn diagnostic_blob(&self) -> Result<Option<Vec<u8>>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let descriptor = self
            .registry_handle
            .registry()
            .stored_descriptor(descriptor.clone())?;
        let (bytes, _) =
            read_adapter_diagnostic_payload("Sampling", self.sampling_id, &descriptor)?;
        Ok(Some(bytes))
    }

    pub fn diagnostic_payload(&self) -> Result<Option<AdapterDiagnosticPayload>> {
        let Some(descriptor) = &self.diagnostics else {
            return Ok(None);
        };
        let descriptor = self
            .registry_handle
            .registry()
            .stored_descriptor(descriptor.clone())?;
        let (_, payload) =
            read_adapter_diagnostic_payload("Sampling", self.sampling_id, &descriptor)?;
        Ok(Some(payload))
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn adapter_options(&self) -> &str {
        &self.adapter_options
    }
}

impl ExperimentDyn {
    pub fn new(name: impl Into<Name>) -> Result<Self> {
        Self::with_registry_handle(LocalRegistryHandle::shared_default()?, name)
    }

    pub fn with_temp_local_registry(name: impl Into<Name>) -> Result<Self> {
        Self::with_registry_handle(LocalRegistryHandle::temp()?, name)
    }

    pub fn with_registry_handle(
        registry_handle: LocalRegistryHandle,
        name: impl Into<Name>,
    ) -> Result<Self> {
        let image_name = name.into().resolve(registry_handle.registry())?;
        Ok(Self {
            registry_handle: registry_handle.clone(),
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Unsealed {
                    state: Some(Box::new(UnsealedExperimentDynState {
                        image_name,
                        subject: None,
                        annotations: HashMap::new(),
                        attachments: AttachmentTable::new(),
                        runs: BTreeMap::new(),
                        next_run_id: 0,
                        autosave: AutosaveController::new(0),
                    })),
                    open_runs: 0,
                },
                registry_handle,
                pending_interrupted_checkpoint: None,
            })),
            interrupted_reason_on_drop: Mutex::new(None),
        })
    }

    pub fn registry_handle(&self) -> LocalRegistryHandle {
        self.registry_handle.clone()
    }

    pub fn load(image_name: crate::artifact::ImageRef) -> Result<Self> {
        Self::from_artifact(LocalArtifactDyn::load(image_name)?)
    }

    pub fn restore_from_checkpoint(image_name: crate::artifact::ImageRef) -> Result<Self> {
        let registry_handle = LocalRegistryHandle::shared_default()?;
        Self::restore_from_checkpoint_in_registry_handle(registry_handle, image_name)
    }

    pub fn restore_from_checkpoint_in_registry_handle(
        registry_handle: LocalRegistryHandle,
        image_name: crate::artifact::ImageRef,
    ) -> Result<Self> {
        let checkpoint = ExperimentCheckpoint::open(
            registry_handle,
            &image_name,
            &[
                super::EXPERIMENT_STATUS_DRAFT,
                super::EXPERIMENT_STATUS_FAILED,
                super::EXPERIMENT_STATUS_INTERRUPTED,
            ],
        )?;
        Self::restore_from_checkpoint_state(checkpoint)
    }

    pub fn import_archive(path: &Path) -> Result<Self> {
        Self::from_artifact(LocalArtifactDyn::import_archive(path)?)
    }

    pub fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let sealed = SealedExperimentDynState::from_artifact(artifact)?;
        Self::from_sealed_state(sealed)
    }

    fn restore_from_checkpoint_state(checkpoint: ExperimentCheckpoint) -> Result<Self> {
        let requested_image_name = checkpoint.requested_image_name()?;
        let sealed =
            SealedExperimentDynState::from_checkpoint_artifact(checkpoint.into_artifact())?;
        Self::from_checkpoint_sealed_state(sealed, requested_image_name)
    }

    fn from_sealed_state(sealed: SealedExperimentDynState) -> Result<Self> {
        let registry_handle = sealed.registry_handle();
        Ok(Self {
            registry_handle: registry_handle.clone(),
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Sealed(sealed),
                registry_handle,
                pending_interrupted_checkpoint: None,
            })),
            interrupted_reason_on_drop: Mutex::new(None),
        })
    }

    fn from_checkpoint_sealed_state(
        sealed: SealedExperimentDynState,
        image_name: ImageRef,
    ) -> Result<Self> {
        let registry_handle = sealed.registry_handle();
        let state = sealed.create_restored_checkpoint_state(image_name)?;
        Ok(Self {
            registry_handle: registry_handle.clone(),
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Unsealed {
                    state: Some(Box::new(state)),
                    open_runs: 0,
                },
                registry_handle,
                pending_interrupted_checkpoint: None,
            })),
            interrupted_reason_on_drop: Mutex::new(None),
        })
    }

    pub fn fork(&self, name: impl Into<Name>) -> Result<Self> {
        let (sealed, registry_handle) = {
            let dyn_state = lock_experiment_state(&self.state);
            let ExperimentDynLifecycle::Sealed(sealed) = &dyn_state.lifecycle else {
                return bail_not_sealed(&dyn_state.lifecycle);
            };
            (sealed.clone(), dyn_state.registry_handle.clone())
        };
        let image_name = name.into().resolve(registry_handle.registry())?;
        let state = sealed.create_forked_child_state(image_name)?;
        Ok(Self {
            registry_handle: registry_handle.clone(),
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Unsealed {
                    state: Some(Box::new(state)),
                    open_runs: 0,
                },
                registry_handle,
                pending_interrupted_checkpoint: None,
            })),
            interrupted_reason_on_drop: Mutex::new(None),
        })
    }

    pub fn is_unsealed(&self) -> bool {
        matches!(
            &lock_experiment_state(&self.state).lifecycle,
            ExperimentDynLifecycle::Unsealed { .. }
        )
    }

    pub fn image_name(&self) -> Result<crate::artifact::ImageRef> {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Unsealed { state, .. } => Ok(state
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
                .image_name
                .clone()),
            ExperimentDynLifecycle::Sealed(sealed) => Ok(sealed.image_name().clone()),
            ExperimentDynLifecycle::Checkpoint { image_name, .. }
            | ExperimentDynLifecycle::CommitFailed { image_name, .. } => Ok(image_name.clone()),
        }
    }

    /// Set a manifest annotation on this unsealed Experiment.
    ///
    /// OMMX-owned annotation keys are reserved. Use caller-owned keys such as
    /// reverse-DNS names for metadata that should appear in registry listings.
    pub fn set_annotation(&self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let key = key.into();
        ensure!(
            !crate::is_reserved_annotation_key(&key),
            "Annotation key `{key}` is reserved for OMMX metadata"
        );
        let mut dyn_state = lock_experiment_state(&self.state);
        ensure_unsealed_for_attachment_write(&dyn_state)?;
        let ExperimentDynLifecycle::Unsealed { state, .. } = &mut dyn_state.lifecycle else {
            return bail_non_unsealed(&dyn_state.lifecycle);
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        state.annotations.insert(key, value.into());
        Ok(())
    }

    /// Set the policy for rolling draft checkpoints after a Run closes.
    ///
    /// The policy can only be changed while this Experiment is unsealed.
    /// Changing it resets its schedule at the current closed-Run count.
    pub fn set_autosave_policy(&self, policy: AutosavePolicy) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.state);
        let ExperimentDynLifecycle::Unsealed { state, .. } = &mut dyn_state.lifecycle else {
            return bail_non_unsealed(&dyn_state.lifecycle);
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let run_count = state.runs.len();
        state.autosave.set_policy(policy, run_count)
    }

    pub fn rename(&self, image_name: crate::artifact::ImageRef) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.state);
        let registry_handle = dyn_state.registry_handle.clone();
        match &mut dyn_state.lifecycle {
            ExperimentDynLifecycle::Unsealed { state, .. } => {
                let state = state
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
                let old_image_name = state.image_name.clone();
                let old_checkpoint_image_name = registry_handle
                    .registry()
                    .experiment_checkpoint_image_name(&old_image_name)?;
                state.image_name = image_name;
                match registry_handle
                    .registry()
                    .remove_image_ref(&old_checkpoint_image_name)
                {
                    Ok(Some(_)) => {
                        let run_count = state.runs.len();
                        state
                            .autosave
                            .record_forced_attempt(std::time::Instant::now());
                        match state.autosave_checkpoint(registry_handle.registry()) {
                            Ok(_) => state.autosave.mark_autosaved(run_count),
                            Err(error) => {
                                tracing::warn!(
                                    error = %error,
                                    "Failed to publish Experiment autosave checkpoint after rename"
                                );
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            checkpoint_image_name = %old_checkpoint_image_name,
                            "Failed to remove Experiment checkpoint ref after rename"
                        );
                    }
                }
                Ok(())
            }
            ExperimentDynLifecycle::Sealed(sealed) => sealed.rename(image_name),
            ExperimentDynLifecycle::Checkpoint {
                image_name: checkpoint_image_name,
                sealed,
            } => {
                sealed.rename(image_name.clone())?;
                *checkpoint_image_name = image_name;
                Ok(())
            }
            lifecycle @ ExperimentDynLifecycle::CommitFailed { .. } => bail_non_unsealed(lifecycle),
        }
    }

    pub fn state_name(&self) -> &'static str {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Unsealed { .. } => "unsealed",
            ExperimentDynLifecycle::Sealed(_) => "sealed",
            ExperimentDynLifecycle::Checkpoint { .. }
            | ExperimentDynLifecycle::CommitFailed { .. } => "failed",
        }
    }

    #[cfg(test)]
    pub(super) fn failure_reason_for_test(&self) -> Option<String> {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Checkpoint { sealed, .. } => {
                sealed.lifecycle.reason().map(ToOwned::to_owned)
            }
            ExperimentDynLifecycle::CommitFailed { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }

    pub fn experiment_status(&self) -> Option<ExperimentStatus> {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Sealed(sealed)
            | ExperimentDynLifecycle::Checkpoint { sealed, .. } => Some(*sealed.lifecycle.status()),
            ExperimentDynLifecycle::CommitFailed { .. } => None,
            ExperimentDynLifecycle::Unsealed { .. } => None,
        }
    }

    /// Concise caller-provided reason for a failed or interrupted Experiment.
    ///
    /// This is lifecycle metadata, not solver diagnostics. Callers should not
    /// include secrets, tracebacks, local variables, or environment values.
    pub fn lifecycle_reason(&self) -> Option<String> {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Sealed(sealed)
            | ExperimentDynLifecycle::Checkpoint { sealed, .. } => {
                sealed.lifecycle.reason().map(ToOwned::to_owned)
            }
            ExperimentDynLifecycle::CommitFailed { .. } => None,
            ExperimentDynLifecycle::Unsealed { .. } => None,
        }
    }

    /// Manifest annotations for this Experiment.
    ///
    /// Unsealed handles return the pending annotations. Sealed handles return
    /// annotations read from the committed artifact manifest.
    pub fn annotations(&self) -> Result<HashMap<String, String>> {
        let dyn_state = lock_experiment_state(&self.state);
        match &dyn_state.lifecycle {
            ExperimentDynLifecycle::Unsealed { state, .. } => Ok(state
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
                .annotations
                .clone()),
            ExperimentDynLifecycle::Sealed(sealed)
            | ExperimentDynLifecycle::Checkpoint { sealed, .. } => sealed.artifact.annotations(),
            lifecycle => bail_not_sealed(lifecycle),
        }
    }

    pub fn open_run_count(&self) -> usize {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Unsealed { open_runs, .. } => *open_runs,
            ExperimentDynLifecycle::Sealed(_)
            | ExperimentDynLifecycle::Checkpoint { .. }
            | ExperimentDynLifecycle::CommitFailed { .. } => 0,
        }
    }
}

impl AttachmentLoggerStorage for &ExperimentDyn {
    type Descriptor = oci_spec::image::Descriptor;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R> {
        let registry_handle = {
            let dyn_state = lock_experiment_state(&self.state);
            ensure_unsealed_for_attachment_write(&dyn_state)?;
            dyn_state.registry_handle.clone()
        };
        f(registry_handle.registry())
    }

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R> {
        let mut dyn_state = lock_experiment_state(&self.state);
        ensure_unsealed_for_attachment_write(&dyn_state)?;
        let ExperimentDynLifecycle::Unsealed { state, .. } = &mut dyn_state.lifecycle else {
            return bail_non_unsealed(&dyn_state.lifecycle);
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        f(&mut state.attachments)
    }

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor> {
        self.with_local_registry(|registry| {
            registry.stored_descriptor(descriptor.clone())?;
            Ok(descriptor)
        })
    }
}

impl ExperimentDyn {
    pub fn commit(&self) -> Result<LocalArtifactDyn> {
        self.suppress_interrupted_on_drop();
        let mut dyn_state = lock_experiment_state(&self.state);
        dyn_state.pending_interrupted_checkpoint = None;
        let (state, open_runs) = match &mut dyn_state.lifecycle {
            ExperimentDynLifecycle::Unsealed { state, open_runs } => (state, open_runs),
            lifecycle => return bail_non_unsealed(lifecycle),
        };
        if *open_runs != 0 {
            crate::bail!("Cannot commit Experiment while {open_runs} Run handle(s) are still open");
        }
        let state = state
            .take()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let image_name = state.image_name.clone();
        let artifact = match state
            .commit(dyn_state.registry_handle.registry())
            .and_then(|_| {
                LocalArtifactDyn::open_in_registry_handle(
                    dyn_state.registry_handle.clone(),
                    image_name.clone(),
                )
            }) {
            Ok(artifact) => artifact,
            Err(error) => {
                let reason = error.to_string();
                dyn_state.lifecycle = ExperimentDynLifecycle::CommitFailed { image_name, reason };
                return Err(error);
            }
        };
        let sealed = match SealedExperimentDynState::from_artifact(artifact.clone()) {
            Ok(sealed) => sealed,
            Err(error) => {
                let reason = error.to_string();
                dyn_state.lifecycle = ExperimentDynLifecycle::CommitFailed { image_name, reason };
                return Err(error);
            }
        };
        dyn_state.lifecycle = ExperimentDynLifecycle::Sealed(sealed);
        Ok(artifact)
    }

    pub fn commit_failed_checkpoint(&self, reason: impl Into<String>) -> Result<()> {
        self.suppress_interrupted_on_drop();
        self.commit_checkpoint(ExperimentLifecycle::Failed {
            reason: Some(reason.into()),
        })
    }

    pub fn commit_interrupted_checkpoint(&self, reason: impl Into<String>) -> Result<()> {
        self.suppress_interrupted_on_drop();
        self.commit_checkpoint(ExperimentLifecycle::Interrupted {
            reason: Some(reason.into()),
        })
    }

    fn commit_checkpoint(&self, lifecycle: ExperimentLifecycle) -> Result<()> {
        let reason = lifecycle
            .reason()
            .expect("explicit failed or interrupted checkpoint has a reason")
            .to_string();
        let mut dyn_state = lock_experiment_state(&self.state);
        dyn_state.pending_interrupted_checkpoint = None;
        let registry_handle = dyn_state.registry_handle.clone();
        let (state, open_runs) = match &mut dyn_state.lifecycle {
            ExperimentDynLifecycle::Unsealed { state, open_runs } => (state, open_runs),
            lifecycle => return bail_non_unsealed(lifecycle),
        };
        if *open_runs != 0 {
            tracing::warn!(
                open_runs = *open_runs,
                "Publishing Experiment checkpoint while Run handle(s) are still open; open-run local state is not included"
            );
        }
        let state = state
            .take()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let image_name = state.image_name.clone();
        let artifact = match state
            .commit_checkpoint(registry_handle.registry(), lifecycle)
            .and_then(|artifact| {
                LocalArtifactDyn::open_in_registry_handle(
                    registry_handle.clone(),
                    artifact.image_name().clone(),
                )
            }) {
            Ok(artifact) => artifact,
            Err(error) => {
                let checkpoint_error = error.to_string();
                dyn_state.lifecycle = ExperimentDynLifecycle::CommitFailed {
                    image_name,
                    reason: format!("{reason}; failed to publish checkpoint: {checkpoint_error}"),
                };
                return Err(error);
            }
        };
        let sealed = match SealedExperimentDynState::from_checkpoint_artifact(artifact) {
            Ok(sealed) => sealed,
            Err(error) => {
                dyn_state.lifecycle = ExperimentDynLifecycle::CommitFailed {
                    image_name,
                    reason: error.to_string(),
                };
                return Err(error);
            }
        };
        dyn_state.lifecycle = ExperimentDynLifecycle::Checkpoint { image_name, sealed };
        Ok(())
    }

    pub fn artifact(&self) -> Result<LocalArtifactDyn> {
        let dyn_state = lock_experiment_state(&self.state);
        match &dyn_state.lifecycle {
            ExperimentDynLifecycle::Sealed(sealed) => Ok(sealed.artifact.clone()),
            ExperimentDynLifecycle::Checkpoint { sealed, .. } => Ok(sealed.artifact.clone()),
            lifecycle => bail_not_sealed(lifecycle),
        }
    }

    fn experiment_attachment_table(&self) -> Result<AttachmentTable<StoredDescriptor<'_>>> {
        let attachments = {
            let dyn_state = lock_experiment_state(&self.state);
            match &dyn_state.lifecycle {
                ExperimentDynLifecycle::Sealed(sealed) => sealed.attachments.clone(),
                ExperimentDynLifecycle::Unsealed { state, .. } => {
                    let state = state
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
                    state.attachments.clone()
                }
                ExperimentDynLifecycle::Checkpoint { sealed, .. } => sealed.attachments.clone(),
                lifecycle => return bail_not_sealed(lifecycle),
            }
        };
        attachments.try_map_owned(|descriptor| {
            self.registry_handle
                .registry()
                .stored_descriptor(descriptor)
        })
    }

    pub fn attachment_names(&self) -> Result<Vec<String>> {
        Ok(self
            .experiment_attachment_table()?
            .names()
            .map(ToOwned::to_owned)
            .collect())
    }

    pub fn attachment_media_type(&self, name: &str) -> Result<MediaType> {
        self.experiment_attachment_table()?.media_type(name)
    }

    pub fn attachment_blob(&self, name: &str) -> Result<Vec<u8>> {
        self.experiment_attachment_table()?.blob(name)
    }

    pub fn attachment_instance(&self, name: &str) -> Result<Instance> {
        self.experiment_attachment_table()?.instance(name)
    }

    pub fn attachment_parametric_instance(&self, name: &str) -> Result<ParametricInstance> {
        self.experiment_attachment_table()?
            .parametric_instance(name)
    }

    pub fn attachment_solution(&self, name: &str) -> Result<Solution> {
        self.experiment_attachment_table()?.solution(name)
    }

    pub fn attachment_sample_set(&self, name: &str) -> Result<SampleSet> {
        self.experiment_attachment_table()?.sample_set(name)
    }

    pub fn write_attachment(
        &self,
        name: &str,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<std::path::PathBuf> {
        self.experiment_attachment_table()?
            .write_attachment(name, path, overwrite)
    }

    pub fn runs(&self) -> Result<Vec<SealedRunDyn>> {
        let dyn_state = lock_experiment_state(&self.state);
        match &dyn_state.lifecycle {
            ExperimentDynLifecycle::Sealed(sealed) => Ok(sealed.runs.values().cloned().collect()),
            ExperimentDynLifecycle::Unsealed { state, .. } => {
                let state = state
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
                Ok(unsealed_run_views(
                    &dyn_state.registry_handle,
                    state.runs.values(),
                ))
            }
            ExperimentDynLifecycle::Checkpoint { sealed, .. } => {
                Ok(sealed.runs.values().cloned().collect())
            }
            lifecycle => bail_not_sealed(lifecycle),
        }
    }

    pub fn run_parameter_cells(&self) -> Result<Vec<RunParameterCell>> {
        let dyn_state = lock_experiment_state(&self.state);
        match &dyn_state.lifecycle {
            ExperimentDynLifecycle::Sealed(sealed) => Ok(sealed.run_parameters.cells()),
            ExperimentDynLifecycle::Unsealed { state, .. } => {
                let state = state
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
                Ok(unsealed_run_parameter_cells(state.runs.values()))
            }
            ExperimentDynLifecycle::Checkpoint { sealed, .. } => Ok(sealed.run_parameters.cells()),
            lifecycle => bail_not_sealed(lifecycle),
        }
    }
}

impl<'reg> AsArtifact<'reg> for ExperimentDyn {
    fn as_artifact(&'reg self) -> Result<LocalArtifact<'reg>> {
        let (image_name, manifest_digest) = {
            let dyn_state = lock_experiment_state(&self.state);
            let artifact = match &dyn_state.lifecycle {
                ExperimentDynLifecycle::Sealed(sealed) => &sealed.artifact,
                ExperimentDynLifecycle::Checkpoint { sealed, .. } => &sealed.artifact,
                lifecycle => return bail_not_sealed(lifecycle),
            };
            (
                artifact.image_name().clone(),
                artifact.manifest_digest().clone(),
            )
        };
        Ok(LocalArtifact::from_parts(
            self.registry_handle.registry(),
            image_name,
            manifest_digest,
        ))
    }
}

impl UnsealedExperimentDynState {
    fn commit<'reg>(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        self.into_unsealed_state(registry)?.commit(registry)
    }

    fn commit_checkpoint<'reg>(
        self,
        registry: &'reg LocalRegistry,
        lifecycle: ExperimentLifecycle,
    ) -> Result<LocalArtifact<'reg>> {
        self.into_unsealed_state(registry)?
            .commit_checkpoint(registry, lifecycle)
    }

    fn autosave_checkpoint<'reg>(
        &mut self,
        registry: &'reg LocalRegistry,
    ) -> Result<LocalArtifact<'reg>> {
        let mut state = self.as_unsealed_state(registry)?;
        state.autosave_checkpoint(registry)
    }

    fn autosave_after_run_close<'reg>(
        &mut self,
        registry: &'reg LocalRegistry,
    ) -> Result<Option<LocalArtifact<'reg>>> {
        let run_count = self.runs.len();
        if !self
            .autosave
            .begin_autosave_attempt(std::time::Instant::now(), run_count)
        {
            return Ok(None);
        }
        let artifact = self.autosave_checkpoint(registry)?;
        self.autosave.mark_autosaved(run_count);
        Ok(Some(artifact))
    }

    fn as_unsealed_state<'reg>(
        &self,
        registry: &'reg LocalRegistry,
    ) -> Result<UnsealedExperimentState<'reg>> {
        Ok(UnsealedExperimentState {
            image_name: self.image_name.clone(),
            subject: self.subject.clone(),
            annotations: self.annotations.clone(),
            attachments: self
                .attachments
                .clone()
                .try_map_owned(|descriptor| registry.stored_descriptor(descriptor))?,
            runs: self
                .runs
                .iter()
                .map(|(run_id, run)| {
                    Ok((
                        *run_id,
                        RunEntry {
                            run_id: run.run_id,
                            lifecycle: run.lifecycle.clone(),
                            attachments: run.attachments.clone().try_map_owned(|descriptor| {
                                registry.stored_descriptor(descriptor)
                            })?,
                            trace: run
                                .trace
                                .clone()
                                .map(|descriptor| registry.stored_descriptor(descriptor))
                                .transpose()?,
                            solves: run
                                .solves
                                .iter()
                                .map(|solve| {
                                    Ok(super::SolveEntry {
                                        solve_id: solve.solve_id,
                                        status: solve.status.clone(),
                                        input: registry.stored_descriptor(solve.input.clone())?,
                                        output: solve
                                            .output
                                            .clone()
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                        adapter: solve.adapter.clone(),
                                        adapter_options: solve.adapter_options.clone(),
                                        diagnostics: solve
                                            .diagnostics
                                            .clone()
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                            samplings: run
                                .samplings
                                .iter()
                                .map(|sampling| {
                                    Ok(super::SamplingEntry {
                                        sampling_id: sampling.sampling_id,
                                        status: sampling.status.clone(),
                                        input: registry
                                            .stored_descriptor(sampling.input.clone())?,
                                        output: sampling
                                            .output
                                            .clone()
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                        adapter: sampling.adapter.clone(),
                                        adapter_options: sampling.adapter_options.clone(),
                                        diagnostics: sampling
                                            .diagnostics
                                            .clone()
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                            parameters: run.parameters.clone(),
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?,
            next_run_id: self.next_run_id,
            autosave: self.autosave.clone(),
        })
    }

    fn into_unsealed_state<'reg>(
        self,
        registry: &'reg LocalRegistry,
    ) -> Result<UnsealedExperimentState<'reg>> {
        Ok(UnsealedExperimentState {
            image_name: self.image_name,
            subject: self.subject,
            annotations: self.annotations,
            attachments: self
                .attachments
                .try_map_owned(|descriptor| registry.stored_descriptor(descriptor))?,
            runs: self
                .runs
                .into_iter()
                .map(|(run_id, run)| {
                    Ok((
                        run_id,
                        RunEntry {
                            run_id: run.run_id,
                            lifecycle: run.lifecycle,
                            attachments: run.attachments.try_map_owned(|descriptor| {
                                registry.stored_descriptor(descriptor)
                            })?,
                            trace: run
                                .trace
                                .map(|descriptor| registry.stored_descriptor(descriptor))
                                .transpose()?,
                            solves: run
                                .solves
                                .into_iter()
                                .map(|solve| {
                                    Ok(super::SolveEntry {
                                        solve_id: solve.solve_id,
                                        status: solve.status,
                                        input: registry.stored_descriptor(solve.input)?,
                                        output: solve
                                            .output
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                        adapter: solve.adapter,
                                        adapter_options: solve.adapter_options,
                                        diagnostics: solve
                                            .diagnostics
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                            samplings: run
                                .samplings
                                .into_iter()
                                .map(|sampling| {
                                    Ok(super::SamplingEntry {
                                        sampling_id: sampling.sampling_id,
                                        status: sampling.status,
                                        input: registry.stored_descriptor(sampling.input)?,
                                        output: sampling
                                            .output
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                        adapter: sampling.adapter,
                                        adapter_options: sampling.adapter_options,
                                        diagnostics: sampling
                                            .diagnostics
                                            .map(|descriptor| {
                                                registry.stored_descriptor(descriptor)
                                            })
                                            .transpose()?,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                            parameters: run.parameters,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?,
            next_run_id: self.next_run_id,
            autosave: self.autosave,
        })
    }
}

impl SealedExperimentDynState {
    fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        Self::from_sealed_experiment(
            SealedExperiment::from_artifact(artifact.clone().as_local_artifact())?,
            artifact,
        )
    }

    fn from_checkpoint_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        Self::from_sealed_experiment(
            SealedExperiment::from_checkpoint_artifact(artifact.clone().as_local_artifact())?,
            artifact,
        )
    }

    fn from_sealed_experiment(
        sealed: SealedExperiment<'_>,
        artifact: LocalArtifactDyn,
    ) -> Result<Self> {
        let registry_handle = artifact.registry_handle();
        let lifecycle = sealed.lifecycle().clone();
        let (attachments, runs, run_parameters) = {
            let attachments = descriptor_attachment_table(sealed.attachment_table());
            let runs = sealed
                .runs()
                .map(|run| {
                    (
                        run.run_id(),
                        SealedRunDyn {
                            registry_handle: registry_handle.clone(),
                            run_id: run.run_id(),
                            lifecycle: run.lifecycle().clone(),
                            attachments: descriptor_attachment_table(run.attachment_table()),
                            trace: run.trace_descriptor().cloned().map(Descriptor::from),
                            solves: run
                                .solves()
                                .iter()
                                .map(|solve| SolveDyn {
                                    registry_handle: registry_handle.clone(),
                                    solve_id: solve.solve_id(),
                                    status: solve.status().clone(),
                                    input: Descriptor::from(solve.input_descriptor().clone()),
                                    output: solve
                                        .output_descriptor()
                                        .cloned()
                                        .map(Descriptor::from),
                                    adapter: solve.adapter().to_string(),
                                    adapter_options: solve.adapter_options().to_string(),
                                    diagnostics: solve
                                        .diagnostic_descriptor()
                                        .cloned()
                                        .map(Descriptor::from),
                                })
                                .collect(),
                            samplings: run
                                .samplings()
                                .iter()
                                .map(|sampling| SamplingDyn {
                                    registry_handle: registry_handle.clone(),
                                    sampling_id: sampling.sampling_id(),
                                    status: sampling.status().clone(),
                                    input: Descriptor::from(sampling.input_descriptor().clone()),
                                    output: sampling
                                        .output_descriptor()
                                        .cloned()
                                        .map(Descriptor::from),
                                    adapter: sampling.adapter().to_string(),
                                    adapter_options: sampling.adapter_options().to_string(),
                                    diagnostics: sampling
                                        .diagnostic_descriptor()
                                        .cloned()
                                        .map(Descriptor::from),
                                })
                                .collect(),
                        },
                    )
                })
                .collect();
            let run_parameters = sealed.run_parameters.clone();
            (attachments, runs, run_parameters)
        };
        Ok(Self {
            lifecycle,
            artifact,
            attachments,
            runs,
            run_parameters,
        })
    }

    fn registry_handle(&self) -> LocalRegistryHandle {
        self.artifact.registry_handle()
    }

    fn create_forked_child_state(
        &self,
        image_name: ImageRef,
    ) -> Result<UnsealedExperimentDynState> {
        self.create_child_state(image_name, HashMap::new())
    }

    fn create_restored_checkpoint_state(
        &self,
        image_name: ImageRef,
    ) -> Result<UnsealedExperimentDynState> {
        self.create_child_state(image_name, self.artifact.annotations()?)
    }

    fn create_child_state(
        &self,
        image_name: ImageRef,
        annotations: HashMap<String, String>,
    ) -> Result<UnsealedExperimentDynState> {
        let subject = Some(
            self.artifact
                .as_local_artifact()
                .stored_manifest_descriptor()?
                .into(),
        );
        let mut parameters_by_run = self.run_parameters.parameter_sets()?;
        let mut runs = BTreeMap::new();

        for run in self.runs.values() {
            let parameters = parameters_by_run
                .remove(&run.run_id)
                .unwrap_or_else(super::parameter::ParameterSet::new);
            let solves = run
                .solves
                .iter()
                .map(|solve| SolveEntryDyn {
                    solve_id: solve.solve_id,
                    status: solve.status.clone(),
                    input: solve.input.clone(),
                    output: solve.output.clone(),
                    adapter: solve.adapter.clone(),
                    adapter_options: solve.adapter_options.clone(),
                    diagnostics: solve.diagnostics.clone(),
                })
                .collect();
            let samplings = run
                .samplings
                .iter()
                .map(|sampling| SamplingEntryDyn {
                    sampling_id: sampling.sampling_id,
                    status: sampling.status.clone(),
                    input: sampling.input.clone(),
                    output: sampling.output.clone(),
                    adapter: sampling.adapter.clone(),
                    adapter_options: sampling.adapter_options.clone(),
                    diagnostics: sampling.diagnostics.clone(),
                })
                .collect();
            runs.insert(
                run.run_id,
                RunEntryDyn {
                    run_id: run.run_id,
                    lifecycle: run.lifecycle.clone(),
                    attachments: run.attachments.clone(),
                    trace: run.trace.clone(),
                    solves,
                    samplings,
                    parameters,
                },
            );
        }

        Ok(UnsealedExperimentDynState {
            image_name,
            subject,
            annotations,
            attachments: self.attachments.clone(),
            next_run_id: next_run_id(runs.keys().copied())?,
            autosave: AutosaveController::new(runs.len()),
            runs,
        })
    }

    fn image_name(&self) -> &ImageRef {
        self.artifact.image_name()
    }

    fn rename(&mut self, image_name: ImageRef) -> Result<()> {
        self.artifact = self.artifact.tag_as(image_name)?;
        Ok(())
    }
}

fn descriptor_attachment_table(
    attachments: &AttachmentTable<StoredDescriptor<'_>>,
) -> AttachmentTable<Descriptor> {
    attachments
        .try_map(|_, descriptor| Ok(Descriptor::from(descriptor.clone())))
        .expect("converting StoredDescriptor to Descriptor should not fail")
}

fn unsealed_run_views<'a>(
    registry_handle: &LocalRegistryHandle,
    runs: impl Iterator<Item = &'a RunEntryDyn>,
) -> Vec<SealedRunDyn> {
    runs.map(|run| SealedRunDyn {
        registry_handle: registry_handle.clone(),
        run_id: run.run_id,
        lifecycle: run.lifecycle.clone(),
        attachments: run.attachments.clone(),
        trace: run.trace.clone(),
        solves: run
            .solves
            .iter()
            .map(|solve| SolveDyn {
                registry_handle: registry_handle.clone(),
                solve_id: solve.solve_id,
                status: solve.status.clone(),
                input: solve.input.clone(),
                output: solve.output.clone(),
                adapter: solve.adapter.clone(),
                adapter_options: solve.adapter_options.clone(),
                diagnostics: solve.diagnostics.clone(),
            })
            .collect(),
        samplings: run
            .samplings
            .iter()
            .map(|sampling| SamplingDyn {
                registry_handle: registry_handle.clone(),
                sampling_id: sampling.sampling_id,
                status: sampling.status.clone(),
                input: sampling.input.clone(),
                output: sampling.output.clone(),
                adapter: sampling.adapter.clone(),
                adapter_options: sampling.adapter_options.clone(),
                diagnostics: sampling.diagnostics.clone(),
            })
            .collect(),
    })
    .collect()
}

fn unsealed_run_parameter_cells<'a>(
    runs: impl Iterator<Item = &'a RunEntryDyn>,
) -> Vec<RunParameterCell> {
    runs.flat_map(|run| {
        run.parameters
            .iter()
            .map(|(name, value)| RunParameterCell {
                run_id: run.run_id,
                name: name.clone(),
                value: value.clone(),
            })
            .collect::<Vec<_>>()
    })
    .collect()
}

fn publish_pending_interrupted_checkpoint(
    registry_handle: LocalRegistryHandle,
    state: Arc<Mutex<ExperimentDynState>>,
    reason: String,
) {
    let experiment = ExperimentDyn {
        registry_handle,
        state,
        interrupted_reason_on_drop: Mutex::new(None),
    };
    if let Err(error) = experiment.commit_interrupted_checkpoint(reason) {
        tracing::warn!(
            error = %error,
            "Failed to publish deferred interrupted Experiment checkpoint after Run close"
        );
    }
}

fn lock_experiment_state(state: &Mutex<ExperimentDynState>) -> MutexGuard<'_, ExperimentDynState> {
    match state.lock() {
        Ok(state) => state,
        Err(poisoned) => {
            tracing::warn!("ExperimentDyn state mutex was poisoned; continuing with inner state");
            poisoned.into_inner()
        }
    }
}

fn store_trace_descriptor(state: &ExperimentDynState, trace: super::Trace) -> Result<Descriptor> {
    let super::Trace { bytes } = trace;
    let descriptor = state.registry_handle.registry().store_layer_blob(
        media_types::trace_otlp_protobuf(),
        &bytes,
        Default::default(),
    )?;
    Ok(Descriptor::from(descriptor))
}

fn ensure_unsealed_for_attachment_write(state: &ExperimentDynState) -> Result<()> {
    match &state.lifecycle {
        ExperimentDynLifecycle::Unsealed { state: Some(_), .. } => Ok(()),
        ExperimentDynLifecycle::Unsealed { state: None, .. } => {
            crate::bail!("Experiment has already been committed")
        }
        other => bail_non_unsealed(other),
    }
}

fn bail_non_unsealed<T>(lifecycle: &ExperimentDynLifecycle) -> Result<T> {
    match lifecycle {
        ExperimentDynLifecycle::Unsealed { state: None, .. } => {
            crate::bail!("Experiment has already been committed")
        }
        ExperimentDynLifecycle::Unsealed { state: Some(_), .. } => {
            unreachable!("unsealed lifecycle was handled by caller")
        }
        ExperimentDynLifecycle::Sealed(_) => crate::bail!("Sealed Experiment is read-only"),
        ExperimentDynLifecycle::Checkpoint { .. } => {
            crate::bail!("Checkpointed Experiment is read-only")
        }
        ExperimentDynLifecycle::CommitFailed { reason, .. } => {
            crate::bail!("Experiment commit has failed: {reason}")
        }
    }
}

fn bail_not_sealed<T>(lifecycle: &ExperimentDynLifecycle) -> Result<T> {
    match lifecycle {
        ExperimentDynLifecycle::Unsealed { .. } => {
            crate::bail!("Experiment must be committed before accessing this view")
        }
        ExperimentDynLifecycle::Sealed(_) => {
            unreachable!("sealed lifecycle was handled by caller")
        }
        ExperimentDynLifecycle::Checkpoint { .. } => {
            unreachable!("checkpoint lifecycle was handled by caller")
        }
        ExperimentDynLifecycle::CommitFailed { reason, .. } => {
            crate::bail!("Experiment commit has failed: {reason}")
        }
    }
}
