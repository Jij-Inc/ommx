//! Serialized Experiment structure stored in the OCI config blob.

use super::{RunEntry, UnsealedExperimentState, EXPERIMENT_SCHEMA_V1};
use crate::artifact::local_registry::StoredDescriptor;
use oci_spec::image::Descriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExperimentConfig {
    pub(super) schema: String,
    pub(super) records: Vec<ExperimentConfigRecord>,
    pub(super) runs: Vec<ExperimentConfigRun>,
    pub(super) run_parameters: Descriptor,
}

impl ExperimentConfig {
    pub(super) fn from_unsealed_state(
        state: &UnsealedExperimentState<'_>,
        run_parameters: &StoredDescriptor<'_>,
    ) -> Self {
        Self {
            schema: EXPERIMENT_SCHEMA_V1.to_string(),
            records: state
                .records
                .iter()
                .map(ExperimentConfigRecord::from_record_ref)
                .collect(),
            runs: state
                .runs
                .values()
                .map(ExperimentConfigRun::from_run_entry)
                .collect(),
            run_parameters: Descriptor::from(run_parameters.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExperimentConfigRun {
    pub(super) run_id: u64,
    pub(super) records: Vec<ExperimentConfigRecord>,
}

impl ExperimentConfigRun {
    fn from_run_entry(run: &RunEntry<'_>) -> Self {
        Self {
            run_id: run.run_id,
            records: run
                .records
                .iter()
                .map(ExperimentConfigRecord::from_record_ref)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExperimentConfigRecord {
    pub(super) name: String,
    pub(super) descriptor: Descriptor,
}

impl ExperimentConfigRecord {
    fn from_record_ref(record: &super::record::RecordRef<'_>) -> Self {
        Self {
            name: record.name().to_string(),
            descriptor: Descriptor::from(record.descriptor().clone()),
        }
    }
}
