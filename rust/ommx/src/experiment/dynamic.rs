//! Dynamic-lifetime Experiment / Run handles.
//!
//! [`Experiment`] and [`Run`] use Rust lifetimes to prove that a run
//! cannot outlive its parent experiment and that registry-backed values
//! cannot outlive their [`LocalRegistry`](crate::artifact::local_registry::LocalRegistry).
//! Dynamic runtimes such as Python cannot carry those lifetimes in
//! their object model, so this module provides owned handles that keep
//! the required registry / parent owners alive at runtime.

use super::{Experiment, ExperimentRecord, Name, ParameterValue, Run, RunParameterCell};
use crate::artifact::{LocalArtifactDyn, LocalRegistryHandle};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::sync::{Arc, Mutex, MutexGuard};

/// Runtime-owned Experiment handle.
///
/// This is the dynamic-lifetime counterpart of [`Experiment`]. It
/// stores the registry owner, the unsealed / sealed state, and the
/// count of still-open [`RunDyn`] handles in Rust SDK code so bindings
/// do not need to duplicate these invariants.
#[derive(Debug, Clone)]
pub struct ExperimentDyn {
    inner: Arc<ExperimentDynInner>,
}

#[derive(Debug)]
struct ExperimentDynInner {
    state: Mutex<ExperimentDynState>,
    registry_handle: LocalRegistryHandle,
}

#[derive(Debug)]
enum ExperimentDynState {
    Unsealed {
        experiment: Option<Box<Experiment<'static>>>,
        open_runs: usize,
    },
    Sealed(super::SealedExperiment<'static>),
}

/// Runtime-owned Run handle.
///
/// Dropping a live `RunDyn` abandons the run and releases the open-run
/// guard. Call [`Self::finish`] to append the run to the parent
/// experiment before dropping it.
#[derive(Debug)]
pub struct RunDyn {
    parent: Arc<ExperimentDynInner>,
    run: Option<Run<'static, 'static>>,
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
        let experiment = Experiment::with_registry(registry_handle.registry(), name)?;
        let experiment = erase_experiment_lifetime(experiment);
        Ok(Self {
            inner: Arc::new(ExperimentDynInner {
                state: Mutex::new(ExperimentDynState::Unsealed {
                    experiment: Some(Box::new(experiment)),
                    open_runs: 0,
                }),
                registry_handle,
            }),
        })
    }

    pub fn load(image_name: crate::artifact::ImageRef) -> Result<Self> {
        Self::from_artifact(LocalArtifactDyn::open(image_name)?)
    }

    pub fn from_artifact(artifact: LocalArtifactDyn) -> Result<Self> {
        let sealed = super::SealedExperiment::from_artifact(artifact.local_artifact().clone())?;
        Ok(Self {
            inner: Arc::new(ExperimentDynInner {
                state: Mutex::new(ExperimentDynState::Sealed(sealed)),
                registry_handle: artifact.registry_handle(),
            }),
        })
    }

    pub fn is_unsealed(&self) -> bool {
        matches!(
            &*self.inner.lock_state(),
            ExperimentDynState::Unsealed { .. }
        )
    }

    pub fn image_name(&self) -> Result<crate::artifact::ImageRef> {
        match &*self.inner.lock_state() {
            ExperimentDynState::Unsealed { experiment, .. } => Ok(experiment
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
                .image_name()),
            ExperimentDynState::Sealed(sealed) => Ok(sealed.image_name().clone()),
        }
    }

    pub fn run(&self) -> Result<RunDyn> {
        let run = {
            let mut state = self.inner.lock_state();
            let ExperimentDynState::Unsealed {
                experiment,
                open_runs,
            } = &mut *state
            else {
                crate::bail!("Sealed Experiment is read-only");
            };
            let experiment = experiment
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
            let run = experiment.run()?;
            *open_runs += 1;
            erase_run_lifetime(run)
        };
        Ok(RunDyn {
            parent: Arc::clone(&self.inner),
            run: Some(run),
        })
    }

    pub fn log_record(
        &self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let state = self.inner.lock_state();
        let ExperimentDynState::Unsealed { experiment, .. } = &*state else {
            crate::bail!("Sealed Experiment is read-only");
        };
        experiment
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
            .log_record(name, media_type, bytes)
    }

    pub fn log_json(&self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let state = self.inner.lock_state();
        let ExperimentDynState::Unsealed { experiment, .. } = &*state else {
            crate::bail!("Sealed Experiment is read-only");
        };
        experiment
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?
            .log_json(name, value)
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
        let mut state = self.inner.lock_state();
        let ExperimentDynState::Unsealed {
            experiment,
            open_runs,
        } = &mut *state
        else {
            crate::bail!("Sealed Experiment is already committed");
        };
        if *open_runs != 0 {
            crate::bail!("Cannot commit Experiment while {open_runs} Run handle(s) are still open");
        }
        let experiment = experiment
            .take()
            .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
        let sealed = experiment.commit()?;
        let artifact = sealed.artifact();
        let artifact =
            LocalArtifactDyn::from_local_artifact(self.inner.registry_handle.clone(), artifact);
        *state = ExperimentDynState::Sealed(sealed);
        Ok(artifact)
    }

    pub fn artifact(&self) -> Result<LocalArtifactDyn> {
        let state = self.inner.lock_state();
        let ExperimentDynState::Sealed(sealed) = &*state else {
            crate::bail!("Experiment must be committed before accessing its artifact");
        };
        Ok(LocalArtifactDyn::from_local_artifact(
            self.inner.registry_handle.clone(),
            sealed.artifact(),
        ))
    }

    pub fn records(&self) -> Result<Vec<ExperimentRecord>> {
        let state = self.inner.lock_state();
        let ExperimentDynState::Sealed(sealed) = &*state else {
            crate::bail!("Experiment must be committed and loaded before using this view");
        };
        Ok(sealed.records().to_vec())
    }

    pub fn run_parameter_cells(&self) -> Result<Vec<RunParameterCell>> {
        let state = self.inner.lock_state();
        let ExperimentDynState::Sealed(sealed) = &*state else {
            crate::bail!("Experiment must be committed and loaded before using this view");
        };
        Ok(sealed.run_parameter_cells())
    }
}

impl RunDyn {
    pub fn run_id(&self) -> Result<u64> {
        Ok(self.open()?.run_id())
    }

    pub fn log_parameter(
        &mut self,
        name: impl Into<String>,
        value: impl Into<ParameterValue>,
    ) -> Result<()> {
        self.open_mut()?.log_parameter(name, value)
    }

    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.open_mut()?.log_record(name, media_type, bytes)
    }

    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        self.open_mut()?.log_json(name, value)
    }

    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.open_mut()?.log_instance(name, instance)
    }

    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.open_mut()?.log_solution(name, solution)
    }

    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.open_mut()?.log_sample_set(name, sample_set)
    }

    pub fn finish(mut self) -> Result<()> {
        let run = self
            .run
            .take()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))?;
        let result = run.finish();
        self.parent.decrement_open_runs();
        result
    }

    pub fn abandon(mut self) {
        if self.run.take().is_some() {
            self.parent.decrement_open_runs();
        }
    }

    fn open(&self) -> Result<&Run<'static, 'static>> {
        self.run
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }

    fn open_mut(&mut self) -> Result<&mut Run<'static, 'static>> {
        self.run
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Run has already been finished"))
    }
}

impl Drop for RunDyn {
    fn drop(&mut self) {
        if self.run.take().is_some() {
            self.parent.decrement_open_runs();
        }
    }
}

impl ExperimentDynInner {
    fn lock_state(&self) -> MutexGuard<'_, ExperimentDynState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!(
                    "ExperimentDyn state mutex was poisoned; continuing with inner state"
                );
                poisoned.into_inner()
            }
        }
    }

    fn decrement_open_runs(&self) {
        let mut state = self.lock_state();
        let ExperimentDynState::Unsealed { open_runs, .. } = &mut *state else {
            tracing::warn!("RunDyn closed after parent ExperimentDyn was sealed");
            return;
        };
        if *open_runs == 0 {
            tracing::warn!("RunDyn open-run counter underflow avoided");
            return;
        }
        *open_runs -= 1;
    }
}

fn erase_experiment_lifetime<'reg>(experiment: Experiment<'reg>) -> Experiment<'static> {
    // The caller stores a `LocalRegistryHandle` in the same
    // `ExperimentDynInner`, and fields are dropped in declaration order:
    // state first, registry handle second. This preserves the erased
    // registry lifetime at runtime.
    unsafe { std::mem::transmute::<Experiment<'reg>, Experiment<'static>>(experiment) }
}

fn erase_run_lifetime<'exp, 'reg>(run: Run<'exp, 'reg>) -> Run<'static, 'static> {
    // The returned `RunDyn` keeps the parent `ExperimentDynInner` alive
    // and increments `open_runs`, preventing the parent experiment from
    // being committed while the erased run handle exists.
    unsafe { std::mem::transmute::<Run<'exp, 'reg>, Run<'static, 'static>>(run) }
}
