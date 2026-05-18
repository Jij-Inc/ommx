//! Experiment / Run handles and run lifecycle.

use super::parameter::ParameterValue;
use super::record::{
    encode_json, json_media_type, store_record_ref, upsert_record_ref, RecordRef, RecordSpace,
};
use super::Experiment;
use crate::artifact::media_types;
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::BTreeMap;
use std::time::Instant;

/// Lifecycle status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunStatus {
    /// The run finished normally.
    Finished,
    /// The run ended via a failure.
    Failed,
}

impl RunStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            RunStatus::Finished => "finished",
            RunStatus::Failed => "failed",
        }
    }
}

/// A handle to a single run within an [`Experiment`].
///
/// A `Run` borrows its parent experiment immutably for `'exp`. It
/// writes payload bytes to the registry CAS immediately, keeps
/// run-scoped records / parameters locally, and writes back to the
/// parent experiment only when [`Self::finish`] or [`Self::fail`]
/// consumes the handle. This lets multiple runs be open at once while
/// Rust prevents committing the parent experiment before live run
/// handles are closed or dropped.
#[derive(Debug)]
pub struct Run<'exp, 'reg> {
    experiment: &'exp Experiment<'reg>,
    run_id: u64,
    records: Vec<RecordRef<'reg>>,
    parameters: BTreeMap<String, ParameterValue>,
    started_at: Instant,
}

/// A closed logical Run recorded in an unsealed Experiment.
///
/// `Run<'exp>` is the live handle: it borrows the parent Experiment and
/// accepts run-scoped records and parameters. `RunEntry` is the row
/// stored by the Experiment after `Run::finish` or `Run::fail` consumes
/// that handle. Commit later projects it to aggregate parameter /
/// attribute tables and record index layers.
#[derive(Debug)]
pub(super) struct RunEntry<'reg> {
    pub(super) run_id: u64,
    pub(super) records: Vec<RecordRef<'reg>>,
    pub(super) parameters: BTreeMap<String, ParameterValue>,
    pub(super) status: RunStatus,
    pub(super) elapsed_secs: f64,
}

impl<'exp, 'reg> Run<'exp, 'reg> {
    pub(super) fn new(experiment: &'exp Experiment<'reg>, run_id: u64) -> Self {
        Self {
            experiment,
            run_id,
            records: Vec::new(),
            parameters: BTreeMap::new(),
            started_at: Instant::now(),
        }
    }

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

    /// Close the run with the `finished` status, record its elapsed
    /// time, and append the closed run state to the parent experiment.
    /// Consumes the handle so no further run-scoped data can be added.
    pub fn finish(self) -> Result<()> {
        self.close(RunStatus::Finished)
    }

    /// Close the run with the `failed` status, record its elapsed time,
    /// and append the closed run state to the parent experiment.
    /// Consumes the handle so no further run-scoped data can be added.
    pub fn fail(self) -> Result<()> {
        self.close(RunStatus::Failed)
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

    fn close(self, status: RunStatus) -> Result<()> {
        let Run {
            experiment,
            run_id,
            records,
            parameters,
            started_at,
        } = self;
        let run = RunEntry {
            run_id,
            records,
            parameters,
            status,
            elapsed_secs: started_at.elapsed().as_secs_f64(),
        };
        experiment.push_closed_run(run)?;
        Ok(())
    }
}

fn validate_parameter_value(name: &str, value: &ParameterValue) -> Result<()> {
    match value {
        ParameterValue::Float(value) if !value.is_finite() => {
            crate::bail!("Run parameter `{name}` float value must be finite")
        }
        _ => Ok(()),
    }
}
