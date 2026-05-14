//! The public `Experiment` / `Run` handles and their `log_*` API.

use super::model::{ExperimentState, Record, RecordKind, RunState, RunStatus, Space};
use super::{
    build_descriptor, commit, ANN_RECORD_KIND, ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE,
    JSON_MEDIA_TYPE,
};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// A mutable experiment session. See the [module documentation](super).
#[derive(Debug, Clone)]
pub struct Experiment {
    pub(super) registry: Arc<LocalRegistry>,
    pub(super) state: Arc<Mutex<ExperimentState>>,
}

/// A handle to a single run within an [`Experiment`].
///
/// `Run` shares the parent experiment's state: `log_*` calls add records
/// to the run space, and [`Run::finish`] / [`Run::fail`] close the run's
/// lifecycle. Cloning a `Run` yields another handle to the same run.
#[derive(Debug, Clone)]
pub struct Run {
    pub(super) registry: Arc<LocalRegistry>,
    pub(super) state: Arc<Mutex<ExperimentState>>,
    pub(super) run_id: u64,
}

impl Experiment {
    /// Start a new experiment session backed by the user's default
    /// Local Registry. The committed artifact is published under an
    /// auto-generated anonymous image name.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let registry = Arc::new(LocalRegistry::open_default()?);
        Ok(Self::with_registry(name, registry, None))
    }

    /// Start a new experiment session against an explicit Local
    /// Registry. When `requested_ref` is set the committed artifact is
    /// published under that image name; otherwise an anonymous image
    /// name is synthesised at commit time.
    pub fn with_registry(
        name: impl Into<String>,
        registry: Arc<LocalRegistry>,
        requested_ref: Option<ImageRef>,
    ) -> Self {
        Experiment {
            registry,
            state: Arc::new(Mutex::new(ExperimentState {
                name: name.into(),
                requested_ref,
                records: Vec::new(),
                runs: Vec::new(),
                next_run_id: 0,
                committed: false,
                artifact: None,
            })),
        }
    }

    /// Start a new [`Run`]. Each run gets a fresh 0-based `run_id`.
    pub fn run(&self) -> Result<Run> {
        let mut state = lock_state(&self.state)?;
        ensure_open(&state)?;
        let run_id = state.next_run_id;
        state.next_run_id += 1;
        state.runs.push(RunState {
            run_id,
            records: Vec::new(),
            status: RunStatus::Running,
            started_at: Instant::now(),
            elapsed_secs: None,
        });
        Ok(Run {
            registry: Arc::clone(&self.registry),
            state: Arc::clone(&self.state),
            run_id,
        })
    }

    /// Record JSON metadata in the experiment space.
    pub fn log_metadata(&self, name: &str, value: serde_json::Value) -> Result<()> {
        let bytes = encode_json(RecordKind::Metadata, name, &value)?;
        self.add_record(RecordKind::Metadata, name, json_media_type(), bytes)
    }

    /// Record a JSON-serialisable object in the experiment space.
    pub fn log_object(&self, name: &str, value: serde_json::Value) -> Result<()> {
        let bytes = encode_json(RecordKind::Object, name, &value)?;
        self.add_record(RecordKind::Object, name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in the experiment space.
    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.add_record(
            RecordKind::Instance,
            name,
            media_types::v1_instance(),
            instance.to_bytes(),
        )
    }

    /// Record a [`Solution`] in the experiment space.
    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.add_record(
            RecordKind::Solution,
            name,
            media_types::v1_solution(),
            solution.to_bytes(),
        )
    }

    /// Record a [`SampleSet`] in the experiment space.
    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.add_record(
            RecordKind::SampleSet,
            name,
            media_types::v1_sample_set(),
            sample_set.to_bytes(),
        )
    }

    fn add_record(
        &self,
        kind: RecordKind,
        name: &str,
        media_type: MediaType,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let mut state = lock_state(&self.state)?;
        ensure_open(&state)?;
        let record = stage_record(
            &self.registry,
            Space::Experiment,
            None,
            kind,
            name,
            media_type,
            &bytes,
        )?;
        upsert_record(&mut state.records, record);
        Ok(())
    }

    /// Seal the session into an immutable OMMX Artifact and publish it
    /// to the Local Registry. Idempotent: a second call returns the
    /// artifact produced by the first.
    pub fn commit(&self) -> Result<LocalArtifact> {
        let mut state = lock_state(&self.state)?;
        if state.committed {
            return state.artifact.clone().ok_or_else(|| {
                crate::error!("Experiment is committed but its artifact is missing")
            });
        }
        let artifact = commit::build_and_publish(&self.registry, &state)?;
        state.committed = true;
        state.artifact = Some(artifact.clone());
        Ok(artifact)
    }

    /// The committed artifact. Errors if [`Experiment::commit`] has not
    /// been called yet.
    pub fn artifact(&self) -> Result<LocalArtifact> {
        let state = lock_state(&self.state)?;
        state
            .artifact
            .clone()
            .ok_or_else(|| crate::error!("Experiment has not been committed yet"))
    }

    /// Whether [`Experiment::commit`] has been called.
    pub fn is_committed(&self) -> Result<bool> {
        Ok(lock_state(&self.state)?.committed)
    }
}

impl Run {
    /// This run's 0-based id within the experiment.
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Record JSON metadata in this run's space.
    pub fn log_metadata(&self, name: &str, value: serde_json::Value) -> Result<()> {
        let bytes = encode_json(RecordKind::Metadata, name, &value)?;
        self.add_record(RecordKind::Metadata, name, json_media_type(), bytes)
    }

    /// Record a JSON-serialisable object in this run's space.
    pub fn log_object(&self, name: &str, value: serde_json::Value) -> Result<()> {
        let bytes = encode_json(RecordKind::Object, name, &value)?;
        self.add_record(RecordKind::Object, name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in this run's space.
    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.add_record(
            RecordKind::Instance,
            name,
            media_types::v1_instance(),
            instance.to_bytes(),
        )
    }

    /// Record a [`Solution`] in this run's space.
    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.add_record(
            RecordKind::Solution,
            name,
            media_types::v1_solution(),
            solution.to_bytes(),
        )
    }

    /// Record a [`SampleSet`] in this run's space.
    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.add_record(
            RecordKind::SampleSet,
            name,
            media_types::v1_sample_set(),
            sample_set.to_bytes(),
        )
    }

    /// Close the run with the `finished` status and record its elapsed
    /// time. A no-op if the run is already closed.
    pub fn finish(&self) -> Result<()> {
        self.close(RunStatus::Finished)
    }

    /// Close the run with the `failed` status and record its elapsed
    /// time. A no-op if the run is already closed.
    pub fn fail(&self) -> Result<()> {
        self.close(RunStatus::Failed)
    }

    fn add_record(
        &self,
        kind: RecordKind,
        name: &str,
        media_type: MediaType,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let mut state = lock_state(&self.state)?;
        ensure_open(&state)?;
        let record = stage_record(
            &self.registry,
            Space::Run,
            Some(self.run_id),
            kind,
            name,
            media_type,
            &bytes,
        )?;
        let run = find_run_mut(&mut state, self.run_id)?;
        upsert_record(&mut run.records, record);
        Ok(())
    }

    fn close(&self, status: RunStatus) -> Result<()> {
        let mut state = lock_state(&self.state)?;
        let run = find_run_mut(&mut state, self.run_id)?;
        if run.status == RunStatus::Running {
            run.elapsed_secs = Some(run.started_at.elapsed().as_secs_f64());
            run.status = status;
        }
        Ok(())
    }
}

fn lock_state(state: &Mutex<ExperimentState>) -> Result<MutexGuard<'_, ExperimentState>> {
    state
        .lock()
        .map_err(|_| crate::error!("Experiment state mutex is poisoned"))
}

fn ensure_open(state: &ExperimentState) -> Result<()> {
    if state.committed {
        crate::bail!("Experiment has already been committed; no further mutation is allowed");
    }
    Ok(())
}

fn find_run_mut(state: &mut ExperimentState, run_id: u64) -> Result<&mut RunState> {
    state
        .runs
        .iter_mut()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| crate::error!("Run {run_id} not found in experiment"))
}

/// Build-phase upsert: a record with the same `(kind, name)` within a
/// space replaces the previous one. Within one `Vec` the space and
/// `run_id` are already fixed, so `(kind, name)` is the remaining key.
fn upsert_record(records: &mut Vec<Record>, record: Record) {
    if let Some(existing) = records
        .iter_mut()
        .find(|r| r.kind == record.kind && r.name == record.name)
    {
        *existing = record;
    } else {
        records.push(record);
    }
}

fn json_media_type() -> MediaType {
    MediaType::Other(JSON_MEDIA_TYPE.to_string())
}

fn encode_json(kind: RecordKind, name: &str, value: &serde_json::Value) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| {
        crate::error!(
            "Failed to encode {} record `{name}` as JSON: {e}",
            kind.as_str()
        )
    })
}

/// Write `bytes` to the registry's BlobStore and build the in-memory
/// [`Record`]: an OCI layer descriptor carrying the experiment / record
/// annotations, plus the matching blob record.
fn stage_record(
    registry: &LocalRegistry,
    space: Space,
    run_id: Option<u64>,
    kind: RecordKind,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<Record> {
    let mut blob = registry.blobs().put_bytes(bytes)?;
    blob.media_type = Some(media_type.to_string());

    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = run_id {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_RECORD_KIND.to_string(), kind.as_str().to_string());
    annotations.insert(ANN_RECORD_NAME.to_string(), name.to_string());

    let descriptor = build_descriptor(media_type, &blob, annotations)?;
    Ok(Record {
        kind,
        name: name.to_string(),
        descriptor,
        blob,
    })
}
