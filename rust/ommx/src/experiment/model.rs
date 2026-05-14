//! In-memory state of an experiment session: the domain state enums and
//! the `RecordRef` / `RunState` / `ExperimentState` structs.

use crate::artifact::{ImageRef, LocalArtifact};
use oci_spec::image::{Descriptor, Digest};
use std::{collections::HashMap, time::Instant};

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
    /// OCI layer descriptor; carries the payload media type and the
    /// experiment / record annotations.
    pub(super) descriptor: Descriptor,
}

/// In-memory state of a single run.
#[derive(Debug)]
pub(super) struct RunState {
    pub(super) run_id: u64,
    pub(super) records: Vec<RecordRef>,
    pub(super) status: RunStatus,
    pub(super) started_at: Instant,
    pub(super) elapsed_secs: Option<f64>,
}

/// In-memory state shared by an [`super::Experiment`] and all its
/// [`super::Run`] handles.
#[derive(Debug)]
pub(super) struct ExperimentState {
    pub(super) name: String,
    /// Image name the committed artifact is published under. `None`
    /// means an anonymous name is synthesised at commit time.
    pub(super) requested_ref: Option<ImageRef>,
    /// Experiment-space records.
    pub(super) records: Vec<RecordRef>,
    pub(super) runs: Vec<RunState>,
    /// CAS-written blobs available for commit-time publication, keyed
    /// by digest.
    pub(super) staged_blobs: HashMap<Digest, Descriptor>,
    pub(super) next_run_id: u64,
    pub(super) committed: bool,
    pub(super) artifact: Option<LocalArtifact>,
}
