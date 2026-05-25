//! Dynamic-lifetime Experiment / Run handles.
//!
//! [`Experiment`] and [`Run`] use Rust lifetimes to prove that a run
//! cannot outlive its parent experiment and that registry-backed values
//! cannot outlive their [`LocalRegistry`](crate::artifact::local_registry::LocalRegistry).
//! Dynamic runtimes such as Python cannot carry those lifetimes in
//! their object model, so this module provides owned handles that keep
//! the required registry / parent owners alive at runtime.

use super::attachment::{
    encode_json, json_media_type, store_attachment_descriptor, AttachmentSpace,
};
use super::{Name, RunEntry, RunParameterCell, SealedExperiment, UnsealedExperimentState};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::{ImageRef, LocalArtifact, LocalArtifactDyn, LocalRegistryHandle};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::{Descriptor, MediaType};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

mod run;

pub use run::RunDyn;

/// Runtime-owned Experiment handle.
///
/// This is the dynamic-lifetime counterpart of [`super::Experiment`].
/// It stores the registry owner, the unsealed / sealed state, and the
/// count of still-open [`RunDyn`] handles in Rust SDK code so bindings
/// do not need to duplicate these invariants.
#[derive(Debug, Clone)]
pub struct ExperimentDyn {
    state: Arc<Mutex<ExperimentDynState>>,
}

#[derive(Debug)]
struct ExperimentDynState {
    lifecycle: ExperimentDynLifecycle,
    registry_handle: LocalRegistryHandle,
}

#[derive(Debug)]
enum ExperimentDynLifecycle {
    Unsealed {
        state: Option<UnsealedExperimentDynState>,
        open_runs: usize,
    },
    Sealed(SealedExperimentDynState),
    Failed {
        image_name: ImageRef,
        reason: String,
    },
}

#[derive(Debug)]
struct UnsealedExperimentDynState {
    image_name: ImageRef,
    attachments: Vec<Descriptor>,
    runs: BTreeMap<u64, RunEntryDyn>,
    next_run_id: u64,
}

#[derive(Debug)]
struct RunEntryDyn {
    run_id: u64,
    attachments: Vec<Descriptor>,
    solves: Vec<SolveEntryDyn>,
    parameters: super::parameter::ParameterSet,
}

#[derive(Debug)]
pub(super) struct SolveEntryDyn {
    pub(super) solve_id: u64,
    pub(super) input: Descriptor,
    pub(super) output: Descriptor,
    pub(super) parameters: super::parameter::ParameterSet,
}

#[derive(Debug, Clone)]
struct SealedExperimentDynState {
    artifact: LocalArtifactDyn,
    attachments: Vec<Descriptor>,
    runs: BTreeMap<u64, SealedRunDyn>,
    run_parameters: super::parameter::RunParameterTable,
}

#[derive(Debug, Clone)]
pub struct SealedRunDyn {
    run_id: u64,
    attachments: Vec<Descriptor>,
    solves: Vec<SealedSolveDyn>,
}

#[derive(Debug, Clone)]
pub struct SealedSolveDyn {
    solve_id: u64,
    input: Descriptor,
    output: Descriptor,
    parameters: BTreeMap<String, super::ParameterValue>,
}

impl SealedRunDyn {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    pub fn attachments(&self) -> &[Descriptor] {
        &self.attachments
    }

    pub fn solves(&self) -> &[SealedSolveDyn] {
        &self.solves
    }
}

impl SealedSolveDyn {
    pub fn solve_id(&self) -> u64 {
        self.solve_id
    }

    pub fn input(&self) -> &Descriptor {
        &self.input
    }

    pub fn output(&self) -> &Descriptor {
        &self.output
    }

    pub fn parameters(&self) -> &BTreeMap<String, super::ParameterValue> {
        &self.parameters
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
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Unsealed {
                    state: Some(UnsealedExperimentDynState {
                        image_name,
                        attachments: Vec::new(),
                        runs: BTreeMap::new(),
                        next_run_id: 0,
                    }),
                    open_runs: 0,
                },
                registry_handle,
            })),
        })
    }

    pub fn load(image_name: crate::artifact::ImageRef) -> Result<Self> {
        Self::from_artifact(LocalArtifactDyn::open(image_name)?)
    }

    pub fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let sealed = SealedExperimentDynState::from_artifact(artifact)?;
        let registry_handle = sealed.registry_handle();
        Ok(Self {
            state: Arc::new(Mutex::new(ExperimentDynState {
                lifecycle: ExperimentDynLifecycle::Sealed(sealed),
                registry_handle,
            })),
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
            ExperimentDynLifecycle::Failed { image_name, .. } => Ok(image_name.clone()),
        }
    }

    pub fn state_name(&self) -> &'static str {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Unsealed { .. } => "unsealed",
            ExperimentDynLifecycle::Sealed(_) => "sealed",
            ExperimentDynLifecycle::Failed { .. } => "failed",
        }
    }

    pub fn open_run_count(&self) -> usize {
        match &lock_experiment_state(&self.state).lifecycle {
            ExperimentDynLifecycle::Unsealed { open_runs, .. } => *open_runs,
            ExperimentDynLifecycle::Sealed(_) | ExperimentDynLifecycle::Failed { .. } => 0,
        }
    }

    pub fn log_attachment(
        &self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.state);
        let descriptor =
            store_experiment_attachment_descriptor(&dyn_state, name, media_type, bytes.as_ref())?;
        let ExperimentDynLifecycle::Unsealed { state, .. } = &mut dyn_state.lifecycle else {
            return bail_non_unsealed(&dyn_state.lifecycle);
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        state.attachments.push(descriptor);
        Ok(())
    }

    pub fn log_json(&self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_attachment(name, json_media_type(), bytes)
    }

    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.log_attachment(
            name,
            crate::artifact::media_types::v1_instance(),
            instance.to_bytes(),
        )
    }

    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.log_attachment(
            name,
            crate::artifact::media_types::v1_solution(),
            solution.to_bytes(),
        )
    }

    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_attachment(
            name,
            crate::artifact::media_types::v1_sample_set(),
            sample_set.to_bytes(),
        )
    }

    pub fn commit(&self) -> Result<LocalArtifactDyn> {
        let mut dyn_state = lock_experiment_state(&self.state);
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
                dyn_state.lifecycle = ExperimentDynLifecycle::Failed { image_name, reason };
                return Err(error);
            }
        };
        let sealed = match SealedExperimentDynState::from_artifact(artifact.clone()) {
            Ok(sealed) => sealed,
            Err(error) => {
                let reason = error.to_string();
                dyn_state.lifecycle = ExperimentDynLifecycle::Failed { image_name, reason };
                return Err(error);
            }
        };
        dyn_state.lifecycle = ExperimentDynLifecycle::Sealed(sealed);
        Ok(artifact)
    }

    pub fn artifact(&self) -> Result<LocalArtifactDyn> {
        let dyn_state = lock_experiment_state(&self.state);
        let ExperimentDynLifecycle::Sealed(sealed) = &dyn_state.lifecycle else {
            return bail_not_sealed(&dyn_state.lifecycle);
        };
        Ok(sealed.artifact.clone())
    }

    pub fn experiment_attachments(&self) -> Result<Vec<Descriptor>> {
        let dyn_state = lock_experiment_state(&self.state);
        let ExperimentDynLifecycle::Sealed(sealed) = &dyn_state.lifecycle else {
            return bail_not_sealed(&dyn_state.lifecycle);
        };
        Ok(sealed.attachments.clone())
    }

    pub fn runs(&self) -> Result<Vec<SealedRunDyn>> {
        let dyn_state = lock_experiment_state(&self.state);
        let ExperimentDynLifecycle::Sealed(sealed) = &dyn_state.lifecycle else {
            return bail_not_sealed(&dyn_state.lifecycle);
        };
        Ok(sealed.runs.values().cloned().collect())
    }

    pub fn run_parameter_cells(&self) -> Result<Vec<RunParameterCell>> {
        let dyn_state = lock_experiment_state(&self.state);
        let ExperimentDynLifecycle::Sealed(sealed) = &dyn_state.lifecycle else {
            return bail_not_sealed(&dyn_state.lifecycle);
        };
        Ok(sealed.run_parameters.cells())
    }
}

impl UnsealedExperimentDynState {
    fn commit<'reg>(self, registry: &'reg LocalRegistry) -> Result<LocalArtifact<'reg>> {
        self.into_unsealed_state(registry)?.commit(registry)
    }

    fn into_unsealed_state<'reg>(
        self,
        registry: &'reg LocalRegistry,
    ) -> Result<UnsealedExperimentState<'reg>> {
        Ok(UnsealedExperimentState {
            image_name: self.image_name,
            attachments: stored_descriptors(registry, self.attachments)?,
            runs: self
                .runs
                .into_iter()
                .map(|(run_id, run)| {
                    Ok((
                        run_id,
                        RunEntry {
                            run_id: run.run_id,
                            attachments: stored_descriptors(registry, run.attachments)?,
                            solves: run
                                .solves
                                .into_iter()
                                .map(|solve| {
                                    Ok(super::SolveEntry {
                                        solve_id: solve.solve_id,
                                        input: registry.stored_descriptor(solve.input)?,
                                        output: registry.stored_descriptor(solve.output)?,
                                        parameters: solve.parameters,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                            parameters: run.parameters,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?,
            next_run_id: self.next_run_id,
        })
    }
}

impl SealedExperimentDynState {
    fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let (attachments, runs, run_parameters) = {
            let sealed = SealedExperiment::from_artifact(artifact.as_local_artifact())?;
            let attachments = descriptors(sealed.experiment_attachments());
            let runs = sealed
                .runs()
                .map(|run| {
                    (
                        run.run_id(),
                        SealedRunDyn {
                            run_id: run.run_id(),
                            attachments: descriptors(run.attachments()),
                            solves: run
                                .solves()
                                .iter()
                                .map(|solve| SealedSolveDyn {
                                    solve_id: solve.solve_id(),
                                    input: Descriptor::from(solve.input().clone()),
                                    output: Descriptor::from(solve.output().clone()),
                                    parameters: solve.parameters().clone(),
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
            artifact,
            attachments,
            runs,
            run_parameters,
        })
    }

    fn registry_handle(&self) -> LocalRegistryHandle {
        self.artifact.registry_handle()
    }

    fn image_name(&self) -> &ImageRef {
        self.artifact.image_name()
    }
}

fn descriptors(attachments: &[StoredDescriptor<'_>]) -> Vec<Descriptor> {
    attachments
        .iter()
        .cloned()
        .map(Descriptor::from)
        .collect::<Vec<_>>()
}

fn stored_descriptors<'reg>(
    registry: &'reg LocalRegistry,
    attachments: Vec<Descriptor>,
) -> Result<Vec<StoredDescriptor<'reg>>> {
    attachments
        .into_iter()
        .map(|descriptor| registry.stored_descriptor(descriptor))
        .collect()
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

fn store_experiment_attachment_descriptor(
    state: &ExperimentDynState,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<Descriptor> {
    ensure_unsealed_for_attachment_write(state)?;
    let descriptor = store_attachment_descriptor(
        state.registry_handle.registry(),
        AttachmentSpace::Experiment,
        name,
        media_type,
        bytes,
    )?;
    Ok(Descriptor::from(descriptor))
}

fn store_run_attachment_descriptor(
    state: &ExperimentDynState,
    run_id: u64,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<Descriptor> {
    ensure_unsealed_for_attachment_write(state)?;
    let descriptor = store_attachment_descriptor(
        state.registry_handle.registry(),
        AttachmentSpace::Run(run_id),
        name,
        media_type,
        bytes,
    )?;
    Ok(Descriptor::from(descriptor))
}

fn store_solve_payload_descriptor(
    state: &ExperimentDynState,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<Descriptor> {
    ensure_unsealed_for_attachment_write(state)?;
    let descriptor =
        state
            .registry_handle
            .registry()
            .store_layer_blob(media_type, bytes, Default::default())?;
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
        ExperimentDynLifecycle::Failed { reason, .. } => {
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
        ExperimentDynLifecycle::Failed { reason, .. } => {
            crate::bail!("Experiment commit has failed: {reason}")
        }
    }
}
