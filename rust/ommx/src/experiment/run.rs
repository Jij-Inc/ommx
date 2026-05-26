//! Experiment / Run handles and run lifecycle.

use super::attachment::{
    encode_json, json_media_type, store_attachment_descriptor, AttachmentSpace,
};
use super::{ParameterValue, Run, RunEntry, SolveEntry};
use crate::artifact::media_types;
use crate::{Instance, ParametricInstance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::BTreeMap;

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

    /// Attach arbitrary bytes with an explicit OCI media type in this
    /// run's space.
    pub fn log_attachment(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_attachment(name, media_type, bytes.as_ref())
    }

    /// Attach a JSON-serialisable value in this run's space.
    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_attachment(name, json_media_type(), bytes)
    }

    /// Attach an [`Instance`] in this run's space.
    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_attachment(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Attach an [`ParametricInstance`] in this run's space.
    pub fn log_parametric_instance(&mut self, name: &str, pi: &ParametricInstance) -> Result<()> {
        self.log_attachment(name, media_types::v1_parametric_instance(), pi.to_bytes())
    }

    /// Attach a [`Solution`] in this run's space.
    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_attachment(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Attach a [`SampleSet`] in this run's space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_attachment(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    /// Log one already-finished solver result under this run.
    ///
    /// The original input [`Instance`] and returned [`Solution`] are
    /// stored as solve-scoped payloads. Solver adapter metadata and
    /// kwargs belong to the solve parameters, not the Run parameter
    /// table.
    pub fn log_finished_solve_result(
        &mut self,
        input: &Instance,
        output: &Solution,
        parameters: BTreeMap<String, String>,
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
            parameters,
        });
        Ok(solve_id)
    }

    /// Close the run and append the closed run state to the parent
    /// experiment. Consumes the handle so no further run-scoped data
    /// can be added.
    pub fn finish(self) -> Result<()> {
        self.close()
    }

    fn add_attachment(&mut self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let descriptor = store_attachment_descriptor(
            self.experiment.registry,
            AttachmentSpace::Run(self.run_id),
            name,
            media_type,
            bytes,
        )?;
        self.attachments.push(descriptor);
        Ok(())
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
