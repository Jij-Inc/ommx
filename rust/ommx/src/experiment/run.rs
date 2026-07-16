//! Experiment / Run handles and run lifecycle.

use super::logging::AttachmentLoggerStorage;
use super::{
    AdapterDiagnosticPayload, AttachmentTable, ParameterValue, Run, RunEntry, RunStatus,
    SamplingEntry, SamplingStatus, SolveEntry, SolveStatus, Trace,
};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::media_types;
use crate::{Instance, SampleSet, Solution};
use anyhow::{ensure, Result};
use oci_spec::image::Descriptor;
use std::collections::HashMap;

/// Data needed to record a finished Solve.
pub struct FinishedSolveRecord<'a> {
    pub input: &'a Instance,
    pub output: &'a Solution,
    pub adapter: String,
    pub adapter_options: String,
    pub diagnostics: Option<AdapterDiagnosticPayload>,
}

/// Data needed to record a finished sampler call.
pub struct FinishedSampleRecord<'a> {
    pub input: &'a Instance,
    pub output: &'a SampleSet,
    pub adapter: String,
    pub adapter_options: String,
    pub diagnostics: Option<AdapterDiagnosticPayload>,
}

/// Data needed to record a failed or interrupted Solve.
pub struct FailedSolveRecord<'a> {
    pub input: &'a Instance,
    pub adapter: String,
    pub adapter_options: String,
    pub status: SolveStatus,
    pub diagnostics: Option<AdapterDiagnosticPayload>,
}

/// Data needed to record a failed or interrupted Sampling.
pub struct FailedSampleRecord<'a> {
    pub input: &'a Instance,
    pub adapter: String,
    pub adapter_options: String,
    pub status: SamplingStatus,
    pub diagnostics: Option<AdapterDiagnosticPayload>,
}

impl<'exp, 'reg> Run<'exp, 'reg> {
    /// This run's 0-based id within the experiment.
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Log a scalar parameter for this run. Parameters are not
    /// Attachments: they are materialised at experiment commit time as a
    /// run-parameter table payload used for comparison views.
    pub fn log_parameter(
        &mut self,
        name: impl Into<String>,
        value: impl Into<ParameterValue>,
    ) -> Result<()> {
        let name = name.into();
        let value = value.into();
        self.parameters.insert(name, value)
    }

    /// Reserve a Solve ID for a solver attempt that will be finalized later.
    pub fn reserve_solve_id(&mut self) -> u64 {
        let solve_id = self.next_solve_id;
        self.next_solve_id += 1;
        solve_id
    }

    /// Reserve a Sampling ID for a sampler attempt that will be finalized later.
    pub fn reserve_sampling_id(&mut self) -> u64 {
        let sampling_id = self.next_sampling_id;
        self.next_sampling_id += 1;
        sampling_id
    }

    /// Log one already-finished solver result with adapter diagnostics.
    ///
    /// The original input [`Instance`] and returned [`Solution`] are
    /// stored as solve-scoped payloads. Solver adapter identity and
    /// adapter options are stored on the Solve entry, not in the Run
    /// parameter table.
    ///
    /// Diagnostics are best-effort metadata. If the diagnostics payload cannot
    /// be encoded or stored, the Solve entry is still recorded without
    /// diagnostics.
    pub fn log_finished_solve(&mut self, record: FinishedSolveRecord<'_>) -> Result<u64> {
        let solve_id = self.reserve_solve_id();
        self.log_finished_solve_with_id(solve_id, record)
    }

    /// Finalize a previously reserved Solve ID as a finished Solve.
    pub fn log_finished_solve_with_id(
        &mut self,
        solve_id: u64,
        record: FinishedSolveRecord<'_>,
    ) -> Result<u64> {
        self.ensure_reserved_solve_id(solve_id)?;
        let FinishedSolveRecord {
            input,
            output,
            adapter,
            adapter_options,
            diagnostics,
        } = record;
        let input = self.experiment.registry.store_instance_layer(input)?;
        let output = self.experiment.registry.store_solution_layer(output)?;
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                self.experiment.registry.store_layer_blob(
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
        self.insert_solve(SolveEntry {
            solve_id,
            status: SolveStatus::Finished,
            input,
            output: Some(output),
            adapter,
            adapter_options,
            diagnostics,
        })?;
        Ok(solve_id)
    }

    /// Log one already-finished sampler result with adapter diagnostics.
    ///
    /// The original input [`Instance`] and returned [`SampleSet`] are stored
    /// as Sampling-scoped payloads. A successful sampling call remains finished
    /// even when the SampleSet contains no feasible samples.
    ///
    /// Diagnostics are best-effort metadata. If the diagnostics payload cannot
    /// be encoded or stored, the Sampling entry is still recorded without
    /// diagnostics.
    pub fn log_finished_sample(&mut self, record: FinishedSampleRecord<'_>) -> Result<u64> {
        let sampling_id = self.reserve_sampling_id();
        self.log_finished_sample_with_id(sampling_id, record)
    }

    /// Finalize a previously reserved Sampling ID with a finished sampler result.
    pub fn log_finished_sample_with_id(
        &mut self,
        sampling_id: u64,
        record: FinishedSampleRecord<'_>,
    ) -> Result<u64> {
        self.ensure_reserved_sampling_id(sampling_id)?;
        let FinishedSampleRecord {
            input,
            output,
            adapter,
            adapter_options,
            diagnostics,
        } = record;
        let input = self.experiment.registry.store_instance_layer(input)?;
        let output = self.experiment.registry.store_sample_set_layer(output)?;
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                self.experiment.registry.store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )
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
        self.insert_sampling(SamplingEntry {
            sampling_id,
            status: SamplingStatus::Finished,
            input,
            output: Some(output),
            adapter,
            adapter_options,
            diagnostics,
        })?;
        Ok(sampling_id)
    }

    /// Log one failed or interrupted sampler call with adapter diagnostics.
    pub fn log_failed_sample(&mut self, record: FailedSampleRecord<'_>) -> Result<u64> {
        ensure!(
            record.status != SamplingStatus::Finished,
            "failed sampler attempt status must not be finished"
        );
        let sampling_id = self.reserve_sampling_id();
        self.log_failed_sample_with_id(sampling_id, record)
    }

    /// Finalize a previously reserved Sampling ID as failed or interrupted.
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
        self.ensure_reserved_sampling_id(sampling_id)?;
        let input = self.experiment.registry.store_instance_layer(input)?;
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                self.experiment.registry.store_layer_blob(
                    media_types::diagnostic_msgpack(),
                    &bytes,
                    HashMap::new(),
                )
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
        self.insert_sampling(SamplingEntry {
            sampling_id,
            status,
            input,
            output: None,
            adapter,
            adapter_options,
            diagnostics,
        })?;
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
        let solve_id = self.reserve_solve_id();
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
        self.ensure_reserved_solve_id(solve_id)?;
        let input = self.experiment.registry.store_instance_layer(input)?;
        let diagnostics = diagnostics.and_then(|diagnostic| {
            match diagnostic.to_msgpack_bytes().and_then(|bytes| {
                self.experiment.registry.store_layer_blob(
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
        self.insert_solve(SolveEntry {
            solve_id,
            status,
            input,
            output: None,
            adapter,
            adapter_options,
            diagnostics,
        })?;
        Ok(solve_id)
    }

    fn ensure_reserved_solve_id(&self, solve_id: u64) -> Result<()> {
        ensure!(
            solve_id < self.next_solve_id,
            "Solve ID {} has not been reserved",
            solve_id
        );
        ensure!(
            !self
                .solves
                .iter()
                .any(|existing| existing.solve_id == solve_id),
            "Run {} already contains Solve {}",
            self.run_id,
            solve_id
        );
        Ok(())
    }

    fn insert_solve(&mut self, solve: SolveEntry<'reg>) -> Result<()> {
        self.ensure_reserved_solve_id(solve.solve_id)?;
        let index = self
            .solves
            .partition_point(|existing| existing.solve_id < solve.solve_id);
        self.solves.insert(index, solve);
        Ok(())
    }

    fn ensure_reserved_sampling_id(&self, sampling_id: u64) -> Result<()> {
        ensure!(
            sampling_id < self.next_sampling_id,
            "Sampling ID {} has not been reserved",
            sampling_id
        );
        ensure!(
            !self
                .samplings
                .iter()
                .any(|existing| existing.sampling_id == sampling_id),
            "Run {} already contains Sampling {}",
            self.run_id,
            sampling_id
        );
        Ok(())
    }

    fn insert_sampling(&mut self, sampling: SamplingEntry<'reg>) -> Result<()> {
        self.ensure_reserved_sampling_id(sampling.sampling_id)?;
        let index = self
            .samplings
            .partition_point(|existing| existing.sampling_id < sampling.sampling_id);
        self.samplings.insert(index, sampling);
        Ok(())
    }

    /// Store a trace for this Run.
    ///
    /// Traces are intentionally not Attachments: they record
    /// execution telemetry for the Run and are referenced from this Run's
    /// Experiment config entry. Rust stores the [`Trace`] payload as
    /// opaque bytes and does not inspect the OpenTelemetry contents.
    pub fn store_trace(&mut self, trace: Trace) -> Result<()> {
        if self.trace.is_some() {
            crate::bail!("Run {} already has a trace", self.run_id);
        }
        let Trace { bytes } = trace;
        let descriptor = self.experiment.registry.store_layer_blob(
            media_types::trace_otlp_protobuf(),
            &bytes,
            Default::default(),
        )?;
        self.trace = Some(descriptor);
        Ok(())
    }

    /// Close the run and append the closed run state to the parent
    /// experiment. Consumes the handle so no further run-scoped data
    /// can be added.
    pub fn finish(self) -> Result<()> {
        self.close(RunStatus::Finished)
    }

    /// Close the run as failed and append the partial run state to the
    /// parent experiment.
    ///
    /// This preserves run-scoped payloads and completed solves or samplings
    /// that were logged before the failure.
    pub fn finish_failed(self) -> Result<()> {
        self.close(RunStatus::Failed)
    }

    /// Close the run as interrupted and append the partial run state to
    /// the parent experiment.
    ///
    /// This is used for user cancellation such as Python
    /// `KeyboardInterrupt`.
    pub fn finish_interrupted(self) -> Result<()> {
        self.close(RunStatus::Interrupted)
    }

    /// Opt into best-effort interrupted finalization if this Run is dropped
    /// before an explicit finish operation.
    ///
    /// Ordinary Runs retain abandon-on-drop behavior. Runs created by
    /// [`super::Experiment::scoped`] and [`super::Experiment::scoped_with_registry`]
    /// opt in automatically. Drop failures are reported through tracing.
    pub fn interrupt_on_drop(mut self) -> Self {
        self.interrupt_on_drop = true;
        self
    }

    fn close(mut self, status: RunStatus) -> Result<()> {
        self.close_inner(status)
    }

    fn close_inner(&mut self, status: RunStatus) -> Result<()> {
        self.closed = true;
        let run = RunEntry {
            run_id: self.run_id,
            status,
            attachments: std::mem::take(&mut self.attachments),
            trace: self.trace.take(),
            solves: std::mem::take(&mut self.solves),
            samplings: std::mem::take(&mut self.samplings),
            parameters: std::mem::take(&mut self.parameters),
        };
        self.experiment.push_closed_run(run)?;
        Ok(())
    }
}

impl Drop for Run<'_, '_> {
    fn drop(&mut self) {
        if !self.interrupt_on_drop || self.closed {
            return;
        }
        if let Err(error) = self.close_inner(RunStatus::Interrupted) {
            tracing::warn!(
                error = %error,
                run_id = self.run_id,
                "Failed to finish interrupted Run during drop"
            );
        }
    }
}

impl<'exp, 'reg> AttachmentLoggerStorage for &mut Run<'exp, 'reg> {
    type Descriptor = StoredDescriptor<'reg>;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R> {
        f(self.experiment.registry)
    }

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R> {
        f(&mut self.attachments)
    }

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor> {
        self.experiment.registry.stored_descriptor(descriptor)
    }
}
