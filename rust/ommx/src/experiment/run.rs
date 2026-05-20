//! Experiment / Run handles and run lifecycle.

use super::record::{
    encode_json, json_media_type, store_record_ref, upsert_record_ref, RecordSpace,
};
use super::{ParameterValue, Run, RunEntry};
use crate::artifact::media_types;
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;

impl<'exp, 'reg> Run<'exp, 'reg> {
    /// This run's 0-based id within the experiment.
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Record a scalar parameter for this run. Parameters are not
    /// Records: they are materialised at experiment commit time as a
    /// run-parameter table payload used for comparison views.
    pub fn log_parameter(
        &mut self,
        name: impl Into<String>,
        value: impl Into<ParameterValue>,
    ) -> Result<()> {
        let name = name.into();
        let value = value.into();
        validate_parameter_value(&name, &value)?;
        self.parameters.insert(name, value);
        Ok(())
    }

    /// Record arbitrary bytes with an explicit OCI media type in this
    /// run's space.
    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_record(name, media_type, bytes.as_ref())
    }

    /// Record a JSON-serialisable value in this run's space.
    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in this run's space.
    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Record a [`Solution`] in this run's space.
    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Record a [`SampleSet`] in this run's space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    /// Close the run and append the closed run state to the parent
    /// experiment. Consumes the handle so no further run-scoped data
    /// can be added.
    pub fn finish(self) -> Result<()> {
        self.close()
    }

    fn add_record(&mut self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let record_ref = store_record_ref(
            self.experiment.registry,
            RecordSpace::Run,
            Some(self.run_id),
            name,
            media_type,
            bytes,
        )?;
        upsert_record_ref(&mut self.records, record_ref);
        Ok(())
    }

    fn close(self) -> Result<()> {
        let Run {
            experiment,
            run_id,
            records,
            parameters,
        } = self;
        let run = RunEntry {
            run_id,
            records,
            parameters,
        };
        experiment.push_closed_run(run)?;
        Ok(())
    }
}

pub(super) fn validate_parameter_value(name: &str, value: &ParameterValue) -> Result<()> {
    match value {
        ParameterValue::Float(value) if !value.is_finite() => {
            crate::bail!("Run parameter `{name}` float value must be finite")
        }
        _ => Ok(()),
    }
}
