//! Experiment / Run handles and run lifecycle.

use super::attachment::{store_attachment_descriptor, AttachmentSpace};
use super::{AttachmentLogger, ParameterValue, Run, RunEntry, SolveEntry};
use crate::artifact::media_types;
use crate::{Instance, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;

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
        output: &Solution,
        adapter: String,
        adapter_options: String,
    ) -> Result<u64> {
        let solve_id = self.next_solve_id;
        self.next_solve_id += 1;
        let input = self.experiment.registry.store_layer_blob(
            media_types::v1_instance(),
            &input.to_bytes(),
            Default::default(),
        )?;
        let output = self.experiment.registry.store_layer_blob(
            media_types::v1_solution(),
            &output.to_bytes(),
            Default::default(),
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

    /// Close the run and append the closed run state to the parent
    /// experiment. Consumes the handle so no further run-scoped data
    /// can be added.
    pub fn finish(self) -> Result<()> {
        self.close()
    }

    fn close(self) -> Result<()> {
        let Run {
            experiment,
            run_id,
            attachments,
            solves,
            next_solve_id: _,
            parameters,
        } = self;
        let run = RunEntry {
            run_id,
            attachments,
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
    ) -> Result<()> {
        let descriptor = store_attachment_descriptor(
            self.experiment.registry,
            AttachmentSpace::Run(self.run_id),
            name,
            media_type,
            bytes.as_ref(),
        )?;
        self.attachments.push(descriptor);
        Ok(())
    }
}
