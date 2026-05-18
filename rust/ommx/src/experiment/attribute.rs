//! Run attribute table serialized as an Experiment aggregate layer.

use super::run::RunEntry;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct RunAttributeTable {
    runs: Vec<RunAttributeRow>,
}

impl RunAttributeTable {
    pub(super) fn from_runs<'reg>(runs: &[RunEntry<'reg>]) -> Self {
        Self {
            runs: runs
                .iter()
                .map(|run| RunAttributeRow {
                    run_id: run.run_id,
                    status: run.status.as_str(),
                    elapsed_seconds: run.elapsed_secs,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct RunAttributeRow {
    run_id: u64,
    status: &'static str,
    elapsed_seconds: f64,
}
