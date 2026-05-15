//! In-memory state of an unsealed experiment session: the domain state
//! enums and the `RecordRef` / `RunEntry` / `UnsealedExperimentState`
//! structs.

use crate::artifact::local_registry::StoredDescriptor;
use crate::artifact::ImageRef;
use serde_json::Value;
use std::collections::BTreeMap;

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

/// A closed logical Run recorded in an unsealed Experiment.
///
/// `Run<'exp>` is the live handle: it borrows the parent Experiment and
/// accepts run-scoped records and parameters. `RunEntry` is the
/// lifetime-free row stored by the Experiment after `Run::finish` or
/// `Run::fail` consumes that handle. Keeping this row-oriented build
/// state preserves the "Run is a logical group inside Experiment"
/// model; commit later projects it to aggregate parameter / attribute
/// tables and record index layers.
#[derive(Debug)]
pub(super) struct RunEntry {
    pub(super) run_id: u64,
    pub(super) records: Vec<RecordRef>,
    pub(super) parameters: BTreeMap<String, Value>,
    pub(super) status: RunStatus,
    pub(super) elapsed_secs: f64,
}

/// Mutable experiment state before the root manifest is sealed. A live
/// [`super::Run`] borrows the parent experiment while it adds
/// run-scoped records. Closed runs are stored as [`RunEntry`] values.
#[derive(Debug)]
pub(super) struct UnsealedExperimentState {
    pub(super) name: String,
    /// Image name the committed artifact is published under. `None`
    /// means an anonymous name is synthesised at commit time.
    pub(super) requested_ref: Option<ImageRef>,
    /// Experiment-space records.
    pub(super) records: Vec<RecordRef>,
    pub(super) runs: Vec<RunEntry>,
    pub(super) next_run_id: u64,
}
