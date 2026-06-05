//! Experiment / Run handles and run lifecycle.

use super::attachment::{read_file_attachment, store_attachment_descriptor};
use super::{AttachmentLogger, ParameterValue, Run, RunEntry, RunStatus, SolveEntry, Trace};
use crate::artifact::{media_types, InstanceAnnotations, SolutionAnnotations};
use crate::{Instance, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::{collections::HashMap, path::Path};

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

    /// Log one already-finished solver result under this run.
    ///
    /// The original input [`Instance`] and returned [`Solution`] are
    /// stored as solve-scoped payloads. Solver adapter identity and
    /// adapter options are stored on the Solve entry, not in the Run
    /// parameter table.
    pub fn log_finished_solve_result(
        &mut self,
        input: &Instance,
        input_annotations: InstanceAnnotations,
        output: &Solution,
        output_annotations: SolutionAnnotations,
        adapter: String,
        adapter_options: String,
    ) -> Result<u64> {
        let solve_id = self.next_solve_id;
        self.next_solve_id += 1;
        let input = self.experiment.registry.store_layer_blob(
            media_types::v1_instance(),
            &input.to_bytes(),
            input_annotations.into_inner(),
        )?;
        let output = self.experiment.registry.store_layer_blob(
            media_types::v1_solution(),
            &output.to_bytes(),
            output_annotations.into_inner(),
        )?;
        self.solves.push(SolveEntry {
            solve_id,
            input,
            output,
            adapter,
            adapter_options,
        });
        Ok(solve_id)
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

impl<'exp, 'reg> AttachmentLogger for &mut Run<'exp, 'reg> {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
    ) -> Result<()> {
        if self.attachments.contains_key(name) {
            crate::bail!("Attachment `{name}` already exists");
        }
        let descriptor = store_attachment_descriptor(
            self.experiment.registry,
            media_type,
            bytes.as_ref(),
            annotations,
        )?;
        self.attachments
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
        if self.attachments.contains_key(name) {
            crate::bail!("Attachment `{name}` already exists");
        }
        let descriptor = store_attachment_descriptor(
            self.experiment.registry,
            media_type,
            bytes.as_ref(),
            HashMap::new(),
        )?;
        self.attachments
            .insert(name.to_string(), descriptor, Some(filename))?;
        Ok(())
    }
}
