//! Serialized Experiment structure stored in the OCI config blob.

use oci_spec::image::Descriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfig {
    pub status: String,
    pub records: Vec<Descriptor>,
    pub runs: Vec<ExperimentConfigRun>,
    pub run_parameters: Descriptor,
}

impl ExperimentConfig {
    pub(crate) fn finished(
        records: Vec<Descriptor>,
        runs: Vec<ExperimentConfigRun>,
        run_parameters: Descriptor,
    ) -> Self {
        Self {
            status: super::EXPERIMENT_STATUS_FINISHED.to_string(),
            records,
            runs,
            run_parameters,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigRun {
    pub run_id: u64,
    pub records: Vec<Descriptor>,
}

impl ExperimentConfigRun {
    pub(crate) fn new(run_id: u64, records: Vec<Descriptor>) -> Self {
        Self { run_id, records }
    }
}
