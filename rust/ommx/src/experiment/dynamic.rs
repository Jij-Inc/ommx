//! Dynamic-lifetime Experiment / Run handles.
//!
//! [`Experiment`] and [`Run`] use Rust lifetimes to prove that a run
//! cannot outlive its parent experiment and that registry-backed values
//! cannot outlive their [`LocalRegistry`](crate::artifact::local_registry::LocalRegistry).
//! Dynamic runtimes such as Python cannot carry those lifetimes in
//! their object model, so this module provides owned handles that keep
//! the required registry / parent owners alive at runtime.

use super::record::{
    encode_json, json_media_type, store_record_ref, upsert_record_ref, RecordSpace,
};
use super::run::validate_parameter_value;
use super::{
    ExperimentRecord, Name, ParameterValue, RunEntry, RunParameterCell, SealedExperiment,
    UnsealedExperimentState,
};
use crate::artifact::{LocalArtifactDyn, LocalRegistryHandle};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Runtime-owned Experiment handle.
///
/// This is the dynamic-lifetime counterpart of [`super::Experiment`].
/// It stores the registry owner, the unsealed / sealed state, and the
/// count of still-open [`RunDyn`] handles in Rust SDK code so bindings
/// do not need to duplicate these invariants.
#[derive(Debug, Clone)]
pub struct ExperimentDyn {
    // `experiment_state` stores registry-backed descriptors whose
    // lifetime is erased to `'static`; keep it before `registry_handle`
    // so it is dropped first.
    experiment_state: Arc<Mutex<ExperimentDynState>>,
    registry_handle: LocalRegistryHandle,
}

#[derive(Debug)]
enum ExperimentDynState {
    Unsealed {
        state: Option<UnsealedExperimentState<'static>>,
        open_runs: usize,
    },
    Sealed(SealedExperiment<'static>),
}

/// Runtime-owned Run handle.
///
/// Dropping a live `RunDyn` abandons the run and releases the open-run
/// guard. Call [`Self::finish`] to append the run to the parent
/// experiment before dropping it.
#[derive(Debug)]
pub struct RunDyn {
    experiment_state: Arc<Mutex<ExperimentDynState>>,
    run_state: Option<RunDynState>,
    registry_handle: LocalRegistryHandle,
}

#[derive(Debug)]
struct RunDynState {
    run_id: u64,
    records: Vec<super::record::RecordRef<'static>>,
    parameters: BTreeMap<String, ParameterValue>,
}

impl ExperimentDyn {
    pub fn new(name: impl Into<Name>) -> Result<Self> {
        Self::with_registry_handle(LocalRegistryHandle::shared_default()?, name)
    }

    pub fn on_temp_local_registry(name: impl Into<Name>) -> Result<Self> {
        Self::with_registry_handle(LocalRegistryHandle::temp()?, name)
    }

    pub fn with_registry_handle(
        registry_handle: LocalRegistryHandle,
        name: impl Into<Name>,
    ) -> Result<Self> {
        let image_name = name.into().resolve(registry_handle.registry())?;
        Ok(Self {
            experiment_state: Arc::new(Mutex::new(ExperimentDynState::Unsealed {
                state: Some(UnsealedExperimentState {
                    image_name,
                    records: Vec::new(),
                    runs: BTreeMap::new(),
                    next_run_id: 0,
                }),
                open_runs: 0,
            })),
            registry_handle,
        })
    }

    pub fn load(image_name: crate::artifact::ImageRef) -> Result<Self> {
        Self::from_artifact(LocalArtifactDyn::open(image_name)?)
    }

    pub fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let sealed = SealedExperiment::from_artifact(artifact.local_artifact().clone())?;
        Ok(Self {
            experiment_state: Arc::new(Mutex::new(ExperimentDynState::Sealed(sealed))),
            registry_handle: artifact.registry_handle(),
        })
    }

    pub fn is_unsealed(&self) -> bool {
        matches!(
            &*lock_experiment_state(&self.experiment_state),
            ExperimentDynState::Unsealed { .. }
        )
    }

    pub fn image_name(&self) -> Result<crate::artifact::ImageRef> {
        match &*lock_experiment_state(&self.experiment_state) {
            ExperimentDynState::Unsealed { state, .. } => Ok(state
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
                .image_name
                .clone()),
            ExperimentDynState::Sealed(sealed) => Ok(sealed.image_name().clone()),
        }
    }

    pub fn run(&self) -> Result<RunDyn> {
        let run = {
            let mut dyn_state = lock_experiment_state(&self.experiment_state);
            let ExperimentDynState::Unsealed { state, open_runs } = &mut *dyn_state else {
                crate::bail!("Sealed Experiment is read-only");
            };
            let state = state
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
            let run_id = state.next_run_id;
            state.next_run_id += 1;
            *open_runs += 1;
            RunDynState {
                run_id,
                records: Vec::new(),
                parameters: BTreeMap::new(),
            }
        };
        Ok(RunDyn {
            experiment_state: Arc::clone(&self.experiment_state),
            run_state: Some(run),
            registry_handle: self.registry_handle.clone(),
        })
    }

    pub fn log_record(
        &self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Unsealed { state, .. } = &mut *dyn_state else {
            crate::bail!("Sealed Experiment is read-only");
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let record_ref = store_record_ref(
            self.registry_handle.registry(),
            RecordSpace::Experiment,
            None,
            name,
            media_type,
            bytes.as_ref(),
        )?;
        let record_ref = erase_record_ref_lifetime(record_ref);
        upsert_record_ref(&mut state.records, record_ref);
        Ok(())
    }

    pub fn log_json(&self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_instance(),
            instance.to_bytes(),
        )
    }

    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_solution(),
            solution.to_bytes(),
        )
    }

    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_sample_set(),
            sample_set.to_bytes(),
        )
    }

    pub fn commit(&self) -> Result<LocalArtifactDyn> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Unsealed { state, open_runs } = &mut *dyn_state else {
            crate::bail!("Sealed Experiment is already committed");
        };
        if *open_runs != 0 {
            crate::bail!("Cannot commit Experiment while {open_runs} Run handle(s) are still open");
        }
        let state = state
            .take()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let artifact = state.commit(self.registry_handle.registry())?;
        let artifact =
            LocalArtifactDyn::from_local_artifact(self.registry_handle.clone(), artifact);
        let sealed = SealedExperiment::from_artifact(artifact.local_artifact().clone())?;
        *dyn_state = ExperimentDynState::Sealed(sealed);
        Ok(artifact)
    }

    pub fn artifact(&self) -> Result<LocalArtifactDyn> {
        let dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Sealed(sealed) = &*dyn_state else {
            crate::bail!("Experiment must be committed before accessing its artifact");
        };
        Ok(LocalArtifactDyn::from_local_artifact(
            self.registry_handle.clone(),
            sealed.artifact(),
        ))
    }

    pub fn records(&self) -> Result<Vec<ExperimentRecord>> {
        let dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Sealed(sealed) = &*dyn_state else {
            crate::bail!("Experiment must be committed and loaded before using this view");
        };
        Ok(sealed.records().to_vec())
    }

    pub fn run_parameter_cells(&self) -> Result<Vec<RunParameterCell>> {
        let dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Sealed(sealed) = &*dyn_state else {
            crate::bail!("Experiment must be committed and loaded before using this view");
        };
        Ok(sealed.run_parameter_cells())
    }
}

impl RunDyn {
    pub fn run_id(&self) -> Result<u64> {
        Ok(self.open()?.run_id)
    }

    pub fn log_parameter(
        &mut self,
        name: impl Into<String>,
        value: impl Into<ParameterValue>,
    ) -> Result<()> {
        let name = name.into();
        let value = value.into();
        validate_parameter_value(&name, &value)?;
        self.open_mut()?.parameters.insert(name, value);
        Ok(())
    }

    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let run_id = self.open()?.run_id;
        let record_ref = store_record_ref(
            self.registry_handle.registry(),
            RecordSpace::Run,
            Some(run_id),
            name,
            media_type,
            bytes.as_ref(),
        )?;
        let record_ref = erase_record_ref_lifetime(record_ref);
        upsert_record_ref(&mut self.open_mut()?.records, record_ref);
        Ok(())
    }

    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_instance(),
            instance.to_bytes(),
        )
    }

    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_solution(),
            solution.to_bytes(),
        )
    }

    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(
            name,
            crate::artifact::media_types::v1_sample_set(),
            sample_set.to_bytes(),
        )
    }

    pub fn finish(mut self) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynState::Unsealed { state, open_runs } = &mut *dyn_state else {
            crate::bail!("Parent Experiment is no longer unsealed");
        };
        let state = state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Parent Experiment has already been committed"))?;
        let run = self
            .run_state
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?;
        if state.runs.contains_key(&run.run_id) {
            decrement_open_runs(open_runs);
            crate::bail!("Run {} has already been recorded", run.run_id);
        }
        state.runs.insert(
            run.run_id,
            RunEntry {
                run_id: run.run_id,
                records: run.records,
                parameters: run.parameters,
            },
        );
        decrement_open_runs(open_runs);
        Ok(())
    }

    pub fn abandon(mut self) {
        if self.run_state.take().is_some() {
            decrement_parent_open_runs(&self.experiment_state);
        }
    }

    fn open(&self) -> Result<&RunDynState> {
        self.run_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }

    fn open_mut(&mut self) -> Result<&mut RunDynState> {
        self.run_state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }
}

impl Drop for RunDyn {
    fn drop(&mut self) {
        if self.run_state.take().is_some() {
            decrement_parent_open_runs(&self.experiment_state);
        }
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

fn decrement_parent_open_runs(state: &Mutex<ExperimentDynState>) {
    let mut state = lock_experiment_state(state);
    let ExperimentDynState::Unsealed { open_runs, .. } = &mut *state else {
        tracing::warn!("RunDyn closed after parent ExperimentDyn was sealed");
        return;
    };
    decrement_open_runs(open_runs);
}

fn decrement_open_runs(open_runs: &mut usize) {
    if *open_runs == 0 {
        tracing::warn!("RunDyn open-run counter underflow avoided");
        return;
    }
    *open_runs -= 1;
}

fn erase_record_ref_lifetime<'reg>(
    record_ref: super::record::RecordRef<'reg>,
) -> super::record::RecordRef<'static> {
    // `ExperimentDyn` / `RunDyn` store the `LocalRegistryHandle` that
    // produced this record descriptor. Their registry-backed state
    // fields are declared before the handle, so erased descriptors are
    // dropped before the registry owner.
    unsafe {
        std::mem::transmute::<super::record::RecordRef<'reg>, super::record::RecordRef<'static>>(
            record_ref,
        )
    }
}
