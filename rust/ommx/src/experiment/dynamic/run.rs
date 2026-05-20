//! Dynamic-lifetime Run handle.

use super::super::record::{encode_json, json_media_type, upsert_record_ref, RecordRef};
use super::super::run::validate_parameter_value;
use super::super::{ParameterValue, RunEntry};
use super::{
    bail_non_unsealed, lock_experiment_state, store_run_record_ref, ExperimentDynLifecycle,
    ExperimentDynState,
};
use crate::artifact::media_types;
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::BTreeMap;
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
    records: Vec<RecordRef<'static>>,
    parameters: BTreeMap<String, ParameterValue>,
}

impl RunDyn {
    pub(in crate::experiment::dynamic) fn from_open_run(
        run_id: u64,
        experiment_state: Arc<Mutex<ExperimentDynState>>,
    ) -> Self {
        Self {
            run_state: Some(RunDynState {
                run_id,
                records: Vec::new(),
                parameters: BTreeMap::new(),
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
        let record_ref = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            store_run_record_ref(&dyn_state, run_id, name, media_type, bytes.as_ref())?
        };
        upsert_record_ref(&mut self.open_mut()?.records, record_ref);
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
