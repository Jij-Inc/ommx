//! Experiment / Run handles and run lifecycle.

use super::logging::AttachmentLoggerStorage;
use super::{
    AttachmentTable, ParameterValue, Run, RunEntry, RunStatus, SolveDiagnosticPayload, SolveEntry,
    SolveStatus, Trace,
};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use crate::artifact::media_types;
use crate::{Instance, Solution};
use anyhow::{ensure, Result};
use oci_spec::image::Descriptor;
use std::collections::HashMap;

/// Data needed to record a finished Solve.
pub struct FinishedSolveRecord<'a> {
    pub input: &'a Instance,
    pub output: &'a Solution,
    pub adapter: String,
    pub adapter_options: String,
    pub diagnostics: Option<SolveDiagnosticPayload>,
}

/// Data needed to record a failed or interrupted Solve.
pub struct FailedSolveRecord<'a> {
    pub input: &'a Instance,
    pub adapter: String,
    pub adapter_options: String,
    pub status: SolveStatus,
    pub diagnostics: Option<SolveDiagnosticPayload>,
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

    /// Log one failed solver call with adapter diagnostics.
    ///
    /// Failed solve attempts have an input, adapter metadata, and optional
    /// diagnostics, but no output Solution.
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
    /// This preserves run-scoped payloads and completed solves that were
    /// logged before the failure.
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

    fn close(self, status: RunStatus) -> Result<()> {
        let Run {
            experiment,
            run_id,
            attachments,
            trace,
            solves,
            next_solve_id: _,
            parameters,
        } = self;
        let run = RunEntry {
            run_id,
            status,
            attachments,
            trace,
            solves,
            parameters,
        };
        experiment.push_closed_run(run)?;
        Ok(())
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
