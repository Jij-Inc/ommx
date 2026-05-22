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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExperimentConfigRun {
    pub run_id: u64,
    pub records: Vec<Descriptor>,
}
