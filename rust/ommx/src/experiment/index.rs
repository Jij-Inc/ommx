//! Experiment index layer serialization.

use super::{RecordRef, UnsealedExperimentState, EXPERIMENT_SCHEMA_V1};
use serde::Serialize;

#[derive(Serialize)]
pub struct ExperimentIndex {
    schema: &'static str,
    name: String,
    experiment_records: Vec<RecordIndexEntry>,
    runs: Vec<RunIndexEntry>,
}

impl ExperimentIndex {
    pub fn from_state(state: &UnsealedExperimentState<'_>) -> Self {
        Self {
            schema: EXPERIMENT_SCHEMA_V1,
            name: state.name.clone(),
            experiment_records: state.records.iter().map(record_index_entry).collect(),
            runs: state
                .runs
                .values()
                .map(|run| RunIndexEntry {
                    run_id: run.run_id,
                    parameter_names: run.parameters.keys().cloned().collect(),
                    records: run.records.iter().map(record_index_entry).collect(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct RunIndexEntry {
    run_id: u64,
    parameter_names: Vec<String>,
    records: Vec<RecordIndexEntry>,
}

#[derive(Serialize)]
struct RecordIndexEntry {
    name: String,
    media_type: String,
    digest: String,
    size: u64,
}

fn record_index_entry(record: &RecordRef<'_>) -> RecordIndexEntry {
    RecordIndexEntry {
        name: record.name.clone(),
        media_type: record.descriptor.media_type().to_string(),
        digest: record.descriptor.digest().to_string(),
        size: record.descriptor.size(),
    }
}
