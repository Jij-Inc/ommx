//! Serialized Experiment structure stored in the OCI config blob.

use super::{RunEntry, UnsealedExperimentState, EXPERIMENT_SCHEMA_V1, EXPERIMENT_STATUS_FINISHED};
use crate::artifact::local_registry::StoredDescriptor;
use oci_spec::image::Descriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExperimentConfig {
    pub(super) schema: String,
    pub(super) status: String,
    pub(super) records: Vec<Descriptor>,
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
            status: EXPERIMENT_STATUS_FINISHED.to_string(),
            records: state.records.iter().map(record_descriptor).collect(),
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
    pub(super) records: Vec<Descriptor>,
}

impl ExperimentConfigRun {
    fn from_run_entry(run: &RunEntry<'_>) -> Self {
        Self {
            run_id: run.run_id,
            records: run.records.iter().map(record_descriptor).collect(),
        }
    }
}

fn record_descriptor(record: &super::record::RecordRef<'_>) -> Descriptor {
    Descriptor::from(record.descriptor().clone())
}
