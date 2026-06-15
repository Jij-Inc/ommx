//! Dynamic-lifetime Run handle.

use super::super::attachment::read_file_attachment;
use super::super::parameter::ParameterSet;
use super::super::{
    AttachmentLogger, AttachmentTable, FailedSolveRecord, FinishedSolveRecord, ParameterValue,
    RunStatus, SolveStatus,
};
use super::{
    bail_non_unsealed, lock_experiment_state, store_run_attachment_descriptor,
    store_solve_payload_descriptor, store_trace_descriptor, ExperimentDyn, ExperimentDynLifecycle,
    ExperimentDynState, RunEntryDyn, SolveEntryDyn,
};
use crate::artifact::media_types;
use anyhow::{ensure, Result};
use oci_spec::image::{Descriptor, MediaType};
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, path::Path};

/// Runtime-owned Run handle.
///
/// Dropping a live `RunDyn` abandons the run and releases the open-run
/// guard. Call [`Self::finish`] to append the run to the parent
/// experiment before dropping it.
///
/// Like the other dynamic experiment handles, `RunDyn` stores raw
/// [`Descriptor`] values internally for registry-backed attachments and
/// solve payloads. The parent `ExperimentDyn` owns the registry handle;
/// when the run is finished, those descriptors are promoted back to
/// [`StoredDescriptor`](crate::artifact::local_registry::StoredDescriptor)
/// values before entering the lifetime-based experiment model.
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
    attachments: AttachmentTable<Descriptor>,
    trace: Option<Descriptor>,
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
            let run_id = super::allocate_next_run_id(&mut state.next_run_id)?;
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
                attachments: AttachmentTable::new(),
                trace: None,
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

    pub fn reserve_solve_id(&mut self) -> Result<u64> {
        let state = self.open_mut()?;
        let solve_id = state.next_solve_id;
        state.next_solve_id += 1;
        Ok(solve_id)
    }

    /// Log one already-finished solver result with adapter diagnostics.
    ///
    /// Diagnostics are best-effort metadata. If the diagnostics payload cannot
    /// be encoded or stored, the Solve entry is still recorded without
    /// diagnostics.
    pub fn log_finished_solve(&mut self, record: FinishedSolveRecord<'_>) -> Result<u64> {
        let solve_id = self.reserve_solve_id()?;
        self.log_finished_solve_with_id(solve_id, record)
    }

    /// Finalize a previously reserved Solve ID as a finished Solve.
    pub fn log_finished_solve_with_id(
        &mut self,
        solve_id: u64,
        record: FinishedSolveRecord<'_>,
    ) -> Result<u64> {
        ensure_reserved_solve_id(self.open()?, solve_id)?;
        let FinishedSolveRecord {
            input,
            input_annotations,
            output,
            output_annotations,
            adapter,
            adapter_options,
            diagnostics,
        } = record;
        let (input_bytes, input_annotations) =
            crate::artifact::encode_instance_layer(input, input_annotations);
        let (output_bytes, output_annotations) =
            crate::artifact::encode_solution_layer(output, output_annotations);
        let (input, output, diagnostics) = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            let input = store_solve_payload_descriptor(
                &dyn_state,
                media_types::v1_instance(),
                &input_bytes,
                input_annotations,
            )?;
            let output = store_solve_payload_descriptor(
                &dyn_state,
                media_types::v1_solution(),
                &output_bytes,
                output_annotations,
            )?;
            let diagnostics = diagnostics.and_then(|diagnostic| {
                match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                    store_solve_payload_descriptor(
                        &dyn_state,
                        media_types::diagnostic_msgpack(),
                        &bytes,
                        HashMap::new(),
                    )
                }) {
                    Ok(descriptor) => Some(descriptor),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "Failed to store Solve diagnostics; recording Solve without diagnostics"
                        );
                        None
                    }
                }
            });
            (input, output, diagnostics)
        };
        let state = self.open_mut()?;
        insert_solve(
            state,
            SolveEntryDyn {
                solve_id,
                status: SolveStatus::Finished,
                input,
                output: Some(output),
                adapter,
                adapter_options,
                diagnostics,
            },
        )?;
        Ok(solve_id)
    }

    /// Log one failed solver call with adapter diagnostics.
    ///
    /// Failed solve attempts have an input, adapter metadata, and optional
    /// diagnostics, but no output Solution.
    pub fn log_failed_solve(&mut self, record: FailedSolveRecord<'_>) -> Result<u64> {
        ensure!(
            record.status != SolveStatus::Finished,
            "failed solve attempt status must not be finished"
        );
        let solve_id = self.reserve_solve_id()?;
        self.log_failed_solve_with_id(solve_id, record)
    }

    /// Finalize a previously reserved Solve ID as a failed or interrupted Solve.
    pub fn log_failed_solve_with_id(
        &mut self,
        solve_id: u64,
        record: FailedSolveRecord<'_>,
    ) -> Result<u64> {
        let FailedSolveRecord {
            input,
            input_annotations,
            adapter,
            adapter_options,
            status,
            diagnostics,
        } = record;
        ensure!(
            status != SolveStatus::Finished,
            "failed solve attempt status must not be finished"
        );
        ensure_reserved_solve_id(self.open()?, solve_id)?;
        let (input_bytes, input_annotations) =
            crate::artifact::encode_instance_layer(input, input_annotations);
        let (input, diagnostics) = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            let input = store_solve_payload_descriptor(
                &dyn_state,
                media_types::v1_instance(),
                &input_bytes,
                input_annotations,
            )?;
            let diagnostics = diagnostics.and_then(|diagnostic| {
                match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                    store_solve_payload_descriptor(
                        &dyn_state,
                        media_types::diagnostic_msgpack(),
                        &bytes,
                        HashMap::new(),
                    )
                }) {
                    Ok(descriptor) => Some(descriptor),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "Failed to store failed Solve diagnostics; recording Solve without diagnostics"
                        );
                        None
                    }
                }
            });
            (input, diagnostics)
        };
        let state = self.open_mut()?;
        insert_solve(
            state,
            SolveEntryDyn {
                solve_id,
                status,
                input,
                output: None,
                adapter,
                adapter_options,
                diagnostics,
            },
        )?;
        Ok(solve_id)
    }

    pub fn store_trace(&mut self, trace: super::super::Trace) -> Result<()> {
        let state = self.open()?;
        if state.trace.is_some() {
            crate::bail!("Run {} already has a trace", state.run_id);
        }
        let descriptor = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            store_trace_descriptor(&dyn_state, trace)?
        };
        self.open_mut()?.trace = Some(descriptor);
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let registry_handle = dyn_state.registry_handle.clone();
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
            crate::bail!("Run {} has already been registered", run.run_id);
        }
        state.runs.insert(
            run.run_id,
            RunEntryDyn {
                run_id: run.run_id,
                status: RunStatus::Finished,
                attachments: run.attachments,
                trace: run.trace,
                solves: run.solves,
                parameters: run.parameters,
            },
        );
        decrement_open_runs(open_runs);
        if let Err(error) = state.autosave_checkpoint(registry_handle.registry()) {
            tracing::warn!(
                error = %error,
                "Failed to publish Experiment autosave checkpoint after Run close"
            );
        }
        Ok(())
    }

    pub fn finish_failed(self) -> Result<()> {
        self.finish_with_status(RunStatus::Failed)
    }

    pub fn finish_interrupted(self) -> Result<()> {
        self.finish_with_status(RunStatus::Interrupted)
    }

    fn finish_with_status(mut self, status: RunStatus) -> Result<()> {
        let mut dyn_state = lock_experiment_state(&self.experiment_state);
        let registry_handle = dyn_state.registry_handle.clone();
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
            crate::bail!("Run {} has already been registered", run.run_id);
        }
        state.runs.insert(
            run.run_id,
            RunEntryDyn {
                run_id: run.run_id,
                status,
                attachments: run.attachments,
                trace: run.trace,
                solves: run.solves,
                parameters: run.parameters,
            },
        );
        decrement_open_runs(open_runs);
        if let Err(error) = state.autosave_checkpoint(registry_handle.registry()) {
            tracing::warn!(
                error = %error,
                "Failed to publish Experiment autosave checkpoint after Run close"
            );
        }
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

impl AttachmentLogger for &mut RunDyn {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
    ) -> Result<()> {
        if self.open()?.attachments.contains_key(name) {
            crate::bail!("Attachment `{name}` already exists");
        }
        let descriptor = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            super::ensure_unsealed_for_attachment_write(&dyn_state)?;
            let registry_handle = dyn_state.registry_handle.clone();
            store_run_attachment_descriptor(
                registry_handle.registry(),
                media_type,
                bytes.as_ref(),
                annotations,
            )?
        };
        self.open_mut()?
            .attachments
            .insert(name.to_string(), descriptor, None)?;
        Ok(())
    }

    fn log_file(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<()> {
        let (media_type, bytes, filename) = read_file_attachment(path, media_type, filename)?;
        if self.open()?.attachments.contains_key(name) {
            crate::bail!("Attachment `{name}` already exists");
        }
        let descriptor = {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            super::ensure_unsealed_for_attachment_write(&dyn_state)?;
            let registry_handle = dyn_state.registry_handle.clone();
            store_run_attachment_descriptor(
                registry_handle.registry(),
                media_type,
                bytes.as_ref(),
                HashMap::new(),
            )?
        };
        self.open_mut()?
            .attachments
            .insert(name.to_string(), descriptor, Some(filename))?;
        Ok(())
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

fn ensure_reserved_solve_id(run: &RunDynState, solve_id: u64) -> Result<()> {
    ensure!(
        solve_id < run.next_solve_id,
        "Solve ID {solve_id} has not been reserved"
    );
    ensure!(
        !run.solves
            .iter()
            .any(|existing| existing.solve_id == solve_id),
        "Run {} already contains Solve {solve_id}",
        run.run_id
    );
    Ok(())
}

fn insert_solve(run: &mut RunDynState, solve: SolveEntryDyn) -> Result<()> {
    ensure_reserved_solve_id(run, solve.solve_id)?;
    let index = run
        .solves
        .partition_point(|existing| existing.solve_id < solve.solve_id);
    run.solves.insert(index, solve);
    Ok(())
}
