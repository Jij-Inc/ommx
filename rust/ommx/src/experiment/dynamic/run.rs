//! Dynamic-lifetime Run handle.

use super::super::logging::AttachmentLoggerStorage;
use super::super::parameter::ParameterSet;
use super::super::{
    AttachmentTable, FailedSampleRecord, FailedSolveRecord, FinishedSampleRecord,
    FinishedSolveRecord, ParameterValue, RunStatus, SamplingStatus, SolveStatus,
};
use super::{
    bail_non_unsealed, ensure_unsealed_for_attachment_write, lock_experiment_state,
    publish_pending_interrupted_checkpoint, store_trace_descriptor, ExperimentDyn,
    ExperimentDynLifecycle, ExperimentDynState, RunEntryDyn, SamplingEntryDyn, SolveEntryDyn,
};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::media_types;
use anyhow::{ensure, Result};
use oci_spec::image::Descriptor;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Runtime-owned Run handle.
///
/// Dropping a live `RunDyn` abandons the run and releases the open-run
/// guard by default. Call [`Self::finish`] to append the run to the parent
/// experiment, or [`Self::interrupt_on_drop`] to opt into best-effort
/// interrupted finalization during drop.
///
/// Like the other dynamic experiment handles, `RunDyn` stores raw
/// [`Descriptor`] values internally for registry-backed attachments and
/// solve and sampling payloads. The parent `ExperimentDyn` owns the registry handle;
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
    interrupt_on_drop: bool,
}

#[derive(Debug)]
struct RunDynState {
    run_id: u64,
    attachments: AttachmentTable<Descriptor>,
    trace: Option<Descriptor>,
    solves: Vec<SolveEntryDyn>,
    next_solve_id: u64,
    samplings: Vec<SamplingEntryDyn>,
    next_sampling_id: u64,
    parameters: ParameterSet,
}

impl ExperimentDyn {
    pub fn run(&self) -> Result<RunDyn> {
        let run_id = {
            let mut dyn_state = lock_experiment_state(&self.state);
            if dyn_state.pending_interrupted_checkpoint.is_some() {
                crate::bail!(
                    "Cannot open a Run while an interrupted Experiment checkpoint is pending"
                );
            }
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

    /// Run one lifecycle-safe callback against a new runtime-owned Run.
    ///
    /// A successful callback finishes the Run. A returned error finishes it as
    /// failed and returns the original callback error. Panic unwind or another
    /// unresolved drop finishes it as interrupted. Partial parameters and
    /// attachments are preserved on failed and interrupted paths.
    pub fn scoped_run<T>(&self, f: impl FnOnce(&mut RunDyn) -> Result<T>) -> Result<T> {
        let mut run = self.run()?.interrupt_on_drop();
        match f(&mut run) {
            Ok(value) => {
                run.finish()?;
                Ok(value)
            }
            Err(error) => {
                if let Err(finish_error) = run.finish_failed_with_reason(error.to_string()) {
                    tracing::warn!(
                        error = %finish_error,
                        "Failed to finish failed Run after callback error"
                    );
                }
                Err(error)
            }
        }
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
                samplings: Vec::new(),
                next_sampling_id: 0,
                parameters: ParameterSet::new(),
            }),
            experiment_state,
            interrupt_on_drop: false,
        }
    }

    /// Opt into best-effort interrupted finalization if this Run is dropped
    /// before an explicit finish operation.
    ///
    /// Drop failures are reported through tracing because [`Drop`] cannot
    /// return them. Ordinary `RunDyn` handles continue to abandon their local
    /// state on unresolved drop.
    pub fn interrupt_on_drop(mut self) -> Self {
        self.interrupt_on_drop = true;
        self
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

    pub fn reserve_sampling_id(&mut self) -> Result<u64> {
        let state = self.open_mut()?;
        let sampling_id = state.next_sampling_id;
        state.next_sampling_id += 1;
        Ok(sampling_id)
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
            output,
            adapter,
            adapter_options,
            diagnostics,
        } = record;
        let registry_handle = self.registry_handle_for_attachment_write()?;
        let registry = registry_handle.registry();
        let input = Descriptor::from(registry.store_instance_layer(input)?);
        let output = Descriptor::from(registry.store_solution_layer(output)?);
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                let registry_handle = self.registry_handle_for_attachment_write()?;
                let descriptor = registry_handle.registry().store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )?;
                Ok(Descriptor::from(descriptor))
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

    /// Log one already-finished sampler result with adapter diagnostics.
    ///
    /// A successful sampling call remains finished even when the SampleSet
    /// contains no feasible samples.
    pub fn log_finished_sample(&mut self, record: FinishedSampleRecord<'_>) -> Result<u64> {
        let sampling_id = self.reserve_sampling_id()?;
        self.log_finished_sample_with_id(sampling_id, record)
    }

    /// Finalize a previously reserved Sampling ID with a finished sampler result.
    pub fn log_finished_sample_with_id(
        &mut self,
        sampling_id: u64,
        record: FinishedSampleRecord<'_>,
    ) -> Result<u64> {
        ensure_reserved_sampling_id(self.open()?, sampling_id)?;
        let FinishedSampleRecord {
            input,
            output,
            adapter,
            adapter_options,
            diagnostics,
        } = record;
        let registry_handle = self.registry_handle_for_attachment_write()?;
        let registry = registry_handle.registry();
        let input = Descriptor::from(registry.store_instance_layer(input)?);
        let output = Descriptor::from(registry.store_sample_set_layer(output)?);
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                let registry_handle = self.registry_handle_for_attachment_write()?;
                let descriptor = registry_handle.registry().store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )?;
                Ok(Descriptor::from(descriptor))
            }) {
                Ok(descriptor) => Some(descriptor),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Failed to store Sampling diagnostics; recording Sampling without diagnostics"
                    );
                    None
                }
            }
        });
        let state = self.open_mut()?;
        insert_sampling(
            state,
            SamplingEntryDyn {
                sampling_id,
                status: SamplingStatus::Finished,
                input,
                output: Some(output),
                adapter,
                adapter_options,
                diagnostics,
            },
        )?;
        Ok(sampling_id)
    }

    pub fn log_failed_sample(&mut self, record: FailedSampleRecord<'_>) -> Result<u64> {
        ensure!(
            record.status != SamplingStatus::Finished,
            "failed sampler attempt status must not be finished"
        );
        let sampling_id = self.reserve_sampling_id()?;
        self.log_failed_sample_with_id(sampling_id, record)
    }

    pub fn log_failed_sample_with_id(
        &mut self,
        sampling_id: u64,
        record: FailedSampleRecord<'_>,
    ) -> Result<u64> {
        let FailedSampleRecord {
            input,
            adapter,
            adapter_options,
            status,
            diagnostics,
        } = record;
        ensure!(
            status != SamplingStatus::Finished,
            "failed sampler attempt status must not be finished"
        );
        ensure_reserved_sampling_id(self.open()?, sampling_id)?;
        let registry_handle = self.registry_handle_for_attachment_write()?;
        let input = Descriptor::from(registry_handle.registry().store_instance_layer(input)?);
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                let registry_handle = self.registry_handle_for_attachment_write()?;
                let descriptor = registry_handle.registry().store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )?;
                Ok(Descriptor::from(descriptor))
            }) {
                Ok(descriptor) => Some(descriptor),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Failed to store failed Sampling diagnostics; recording Sampling without diagnostics"
                    );
                    None
                }
            }
        });
        let state = self.open_mut()?;
        insert_sampling(
            state,
            SamplingEntryDyn {
                sampling_id,
                status,
                input,
                output: None,
                adapter,
                adapter_options,
                diagnostics,
            },
        )?;
        Ok(sampling_id)
    }

    /// Log one failed solver call with adapter diagnostics.
    ///
    /// Failed solve attempts have an input, adapter metadata, and optional
    /// diagnostics, but no output.
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
        let registry_handle = self.registry_handle_for_attachment_write()?;
        let input = Descriptor::from(registry_handle.registry().store_instance_layer(input)?);
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                let registry_handle = self.registry_handle_for_attachment_write()?;
                let descriptor = registry_handle.registry().store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )?;
                Ok(Descriptor::from(descriptor))
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
        self.interrupt_on_drop = false;
        self.close_with_status(RunStatus::Finished, None)
    }

    pub fn finish_failed(self) -> Result<()> {
        self.finish_with_status(RunStatus::Failed, None)
    }

    /// Close the run as failed with a concise durable reason.
    pub fn finish_failed_with_reason(self, reason: impl Into<String>) -> Result<()> {
        self.finish_with_status(RunStatus::Failed, Some(reason.into()))
    }

    pub fn finish_interrupted(self) -> Result<()> {
        self.finish_with_status(RunStatus::Interrupted, None)
    }

    /// Close the run as interrupted with a concise durable reason.
    pub fn finish_interrupted_with_reason(self, reason: impl Into<String>) -> Result<()> {
        self.finish_with_status(RunStatus::Interrupted, Some(reason.into()))
    }

    fn finish_with_status(mut self, status: RunStatus, reason: Option<String>) -> Result<()> {
        self.interrupt_on_drop = false;
        self.close_with_status(status, reason)
    }

    fn close_with_status(&mut self, status: RunStatus, reason: Option<String>) -> Result<()> {
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
                reason,
                attachments: run.attachments,
                trace: run.trace,
                solves: run.solves,
                samplings: run.samplings,
                parameters: run.parameters,
            },
        );
        decrement_open_runs(open_runs);
        if let Err(error) = state.autosave_after_run_close(registry_handle.registry()) {
            tracing::warn!(
                error = %error,
                "Failed to publish Experiment autosave checkpoint after Run close"
            );
        }
        let pending_interrupted_checkpoint = if *open_runs == 0 {
            dyn_state.pending_interrupted_checkpoint.clone()
        } else {
            None
        };
        drop(dyn_state);
        if let Some(reason) = pending_interrupted_checkpoint {
            publish_pending_interrupted_checkpoint(
                registry_handle,
                Arc::clone(&self.experiment_state),
                reason,
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

    fn registry_handle_for_attachment_write(&self) -> Result<crate::artifact::LocalRegistryHandle> {
        let dyn_state = lock_experiment_state(&self.experiment_state);
        ensure_unsealed_for_attachment_write(&dyn_state)?;
        Ok(dyn_state.registry_handle.clone())
    }
}

impl AttachmentLoggerStorage for &mut RunDyn {
    type Descriptor = oci_spec::image::Descriptor;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R> {
        let registry_handle = self.registry_handle_for_attachment_write()?;
        f(registry_handle.registry())
    }

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R> {
        {
            let dyn_state = lock_experiment_state(&self.experiment_state);
            ensure_unsealed_for_attachment_write(&dyn_state)?;
        }
        f(&mut self.open_mut()?.attachments)
    }

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor> {
        let registry_handle = self.registry_handle_for_attachment_write()?;
        registry_handle
            .registry()
            .stored_descriptor(descriptor.clone())?;
        Ok(descriptor)
    }
}

impl Drop for RunDyn {
    fn drop(&mut self) {
        if self.run_state.is_none() {
            return;
        }
        if self.interrupt_on_drop {
            if let Err(error) = self.close_with_status(RunStatus::Interrupted, None) {
                tracing::warn!(
                    error = %error,
                    "Failed to finish interrupted Run during drop"
                );
            }
        } else if self.run_state.take().is_some() {
            decrement_parent_open_runs(&self.experiment_state);
        }
    }
}

fn decrement_parent_open_runs(state: &Arc<Mutex<ExperimentDynState>>) {
    let mut experiment = lock_experiment_state(state);
    let registry_handle = experiment.registry_handle.clone();
    let ExperimentDynLifecycle::Unsealed { open_runs, .. } = &mut experiment.lifecycle else {
        tracing::warn!("RunDyn closed after parent ExperimentDyn was sealed");
        return;
    };
    decrement_open_runs(open_runs);
    let pending_interrupted_checkpoint = if *open_runs == 0 {
        experiment.pending_interrupted_checkpoint.clone()
    } else {
        None
    };
    drop(experiment);
    if let Some(reason) = pending_interrupted_checkpoint {
        publish_pending_interrupted_checkpoint(registry_handle, Arc::clone(state), reason);
    }
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

fn ensure_reserved_sampling_id(run: &RunDynState, sampling_id: u64) -> Result<()> {
    ensure!(
        sampling_id < run.next_sampling_id,
        "Sampling ID {sampling_id} has not been reserved"
    );
    ensure!(
        !run.samplings
            .iter()
            .any(|existing| existing.sampling_id == sampling_id),
        "Run {} already contains Sampling {sampling_id}",
        run.run_id
    );
    Ok(())
}

fn insert_sampling(run: &mut RunDynState, sampling: SamplingEntryDyn) -> Result<()> {
    ensure_reserved_sampling_id(run, sampling.sampling_id)?;
    let index = run
        .samplings
        .partition_point(|existing| existing.sampling_id < sampling.sampling_id);
    run.samplings.insert(index, sampling);
    Ok(())
}
