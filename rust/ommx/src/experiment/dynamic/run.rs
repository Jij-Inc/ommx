//! Dynamic-lifetime Run handle.

use super::super::parameter::ParameterSet;
use super::super::record::{encode_json, json_media_type};
use super::super::ParameterValue;
use super::{
    bail_non_unsealed, lock_experiment_state, store_run_record_descriptor,
    store_solve_payload_descriptor, ExperimentDyn, ExperimentDynLifecycle, ExperimentDynState,
    RunEntryDyn, SolveEntryDyn,
};
use crate::artifact::media_types;
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::{Descriptor, MediaType};
use std::sync::{Arc, Mutex};

/// Runtime-owned Run handle.
///
/// Dropping a live `RunDyn` abandons the run and releases the open-run
/// guard. Call [`Self::finish`] to append the run to the parent
/// experiment before dropping it.
#[derive(Debug)]
pub struct RunDyn {
    // Run-scoped registry-backed descriptors must be dropped before
    // releasing the parent Experiment state that owns the registry
    // handle.
    run_state: Option<RunDynState>,
    experiment_state: Arc<Mutex<ExperimentDynState>>,
}

#[derive(Debug)]
struct RunDynState {
    run_id: u64,
    records: Vec<Descriptor>,
    solves: Vec<SolveEntryDyn>,
    next_solve_id: u64,
    parameters: ParameterSet,
}

impl ExperimentDyn {
    pub fn run(&self) -> Result<RunDyn> {
        let run_id = {
            let mut dyn_state = lock_experiment_state(&self.state);
            let ExperimentDynLifecycle::Unsealed { state, open_runs } = &mut dyn_state.lifecycle
            else {
                return bail_non_unsealed(&dyn_state.lifecycle);
            };
            let state = state
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Experiment has already been committed"))?;
            let run_id = state.next_run_id;
            state.next_run_id += 1;
            *open_runs += 1;
            run_id
        };
        Ok(RunDyn::from_open_run(run_id, Arc::clone(&self.state)))
    }
}

impl RunDyn {
    fn from_open_run(run_id: u64, experiment_state: Arc<Mutex<ExperimentDynState>>) -> Self {
        Self {
            run_state: Some(RunDynState {
                run_id,
                records: Vec::new(),
                solves: Vec::new(),
                next_solve_id: 0,
                parameters: ParameterSet::new(),
            }),
            experiment_state,
        }
    }

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
        self.open_mut()?.parameters.insert(name, value)
    }

    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let run_id = self.open()?.run_id;
        let descriptor = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            store_run_record_descriptor(&dyn_state, run_id, name, media_type, bytes.as_ref())?
        };
        self.open_mut()?.records.push(descriptor);
        Ok(())
    }

    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    pub fn log_solve(
        &mut self,
        input: &Instance,
        output: &Solution,
        parameters: impl IntoIterator<Item = (String, ParameterValue)>,
    ) -> Result<u64> {
        let solve_id = self.open()?.next_solve_id;
        let (input, output) = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            (
                store_solve_payload_descriptor(
                    &dyn_state,
                    media_types::v1_instance(),
                    &input.to_bytes(),
                )?,
                store_solve_payload_descriptor(
                    &dyn_state,
                    media_types::v1_solution(),
                    &output.to_bytes(),
                )?,
            )
        };
        let state = self.open_mut()?;
        state.next_solve_id += 1;
        state.solves.push(SolveEntryDyn {
            solve_id,
            input,
            output,
            parameters: ParameterSet::from_entries(parameters)?,
        });
        Ok(solve_id)
    }

    pub fn finish(mut self) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let ExperimentDynLifecycle::Unsealed { state, open_runs } = &mut dyn_state.lifecycle else {
            return bail_non_unsealed(&dyn_state.lifecycle);
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
            RunEntryDyn {
                run_id: run.run_id,
                records: run.records,
                solves: run.solves,
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

fn decrement_parent_open_runs(state: &Mutex<ExperimentDynState>) {
    let mut state = lock_experiment_state(state);
    let ExperimentDynLifecycle::Unsealed { open_runs, .. } = &mut state.lifecycle else {
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
