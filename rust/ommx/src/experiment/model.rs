//! In-memory state of an unsealed experiment session: the domain state
//! enums and the `RecordRef` / `RunState` / `UnsealedExperimentState`
//! structs.

use crate::artifact::local_registry::StoredDescriptor;
use crate::artifact::ImageRef;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

/// The storage space a [`RecordRef`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Space {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single run.
    Run,
}

impl Space {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Space::Experiment => "experiment",
            Space::Run => "run",
        }
    }
}

/// Lifecycle status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunStatus {
    /// The run is open and still accepting records.
    Running,
    /// The run finished normally.
    Finished,
    /// The run ended via a failure.
    Failed,
}

impl RunStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Failed => "failed",
        }
    }
}

/// A named reference to a payload that has already been written to the
/// BlobStore.
#[derive(Debug, Clone)]
pub(super) struct RecordRef {
    pub(super) name: String,
    /// OCI layer descriptor whose payload bytes are present in the
    /// Local Registry BlobStore. Carries the payload media type and
    /// the experiment / record annotations.
    pub(super) descriptor: StoredDescriptor,
}

/// In-memory state of a single run.
#[derive(Debug)]
pub(super) struct RunState {
    pub(super) run_id: u64,
    pub(super) records: Vec<RecordRef>,
    pub(super) parameters: BTreeMap<String, Value>,
    pub(super) status: RunStatus,
    pub(super) started_at: Instant,
    pub(super) elapsed_secs: Option<f64>,
}

/// Mutable experiment state before the root manifest is sealed. A live
/// [`super::Run`] mutably borrows the parent experiment while it adds
/// run-scoped records or closes the run lifecycle.
#[derive(Debug)]
pub(super) struct UnsealedExperimentState {
    pub(super) name: String,
    /// Image name the committed artifact is published under. `None`
    /// means an anonymous name is synthesised at commit time.
    pub(super) requested_ref: Option<ImageRef>,
    /// Experiment-space records.
    pub(super) records: Vec<RecordRef>,
    pub(super) runs: Vec<RunState>,
    pub(super) next_run_id: u64,
}
