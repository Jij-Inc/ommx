//! The public `Experiment` / `Run` handles and their `log_*` API.

use super::model::{ExperimentState, RecordRef, RunState, RunStatus, Space};
use super::{build_descriptor, commit, ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{BlobRecord, LocalRegistry};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// OCI layer media type for JSON record payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

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
                staged_blobs: HashMap::new(),
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

    /// Record arbitrary bytes with an explicit OCI media type in the
    /// experiment space.
    pub fn log_record(
        &self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_record(name, media_type, bytes.as_ref())
    }

    /// Record a JSON-serialisable value in the experiment space.
    pub fn log_json(&self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in the experiment space.
    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Record a [`Solution`] in the experiment space.
    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Record a [`SampleSet`] in the experiment space.
    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    fn add_record(&self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let mut state = lock_state(&self.state)?;
        ensure_open(&state)?;
        let (record_ref, blob) = stage_record_ref(
            &self.registry,
            Space::Experiment,
            None,
            name,
            media_type,
            bytes,
        )?;
        remember_staged_blob(&mut state, blob);
        upsert_record_ref(&mut state.records, record_ref);
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

    /// Record arbitrary bytes with an explicit OCI media type in this
    /// run's space.
    pub fn log_record(
        &self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_record(name, media_type, bytes.as_ref())
    }

    /// Record a JSON-serialisable value in this run's space.
    pub fn log_json(&self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in this run's space.
    pub fn log_instance(&self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Record a [`Solution`] in this run's space.
    pub fn log_solution(&self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Record a [`SampleSet`] in this run's space.
    pub fn log_sample_set(&self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
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

    fn add_record(&self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let mut state = lock_state(&self.state)?;
        ensure_open(&state)?;
        let (record_ref, blob) = stage_record_ref(
            &self.registry,
            Space::Run,
            Some(self.run_id),
            name,
            media_type,
            bytes,
        )?;
        remember_staged_blob(&mut state, blob);
        let run = find_run_mut(&mut state, self.run_id)?;
        upsert_record_ref(&mut run.records, record_ref);
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

fn remember_staged_blob(state: &mut ExperimentState, blob: BlobRecord) {
    state
        .staged_blobs
        .entry(blob.digest.clone())
        .or_insert(blob);
}

/// Build-phase upsert: a record with the same `(media_type, name)`
/// within a space replaces the previous one. Within one `Vec` the space
/// and `run_id` are already fixed, so `(media_type, name)` is the
/// remaining key.
fn upsert_record_ref(records: &mut Vec<RecordRef>, record_ref: RecordRef) {
    if let Some(existing) = records.iter_mut().find(|r| {
        r.descriptor.media_type() == record_ref.descriptor.media_type() && r.name == record_ref.name
    }) {
        *existing = record_ref;
    } else {
        records.push(record_ref);
    }
}

fn json_media_type() -> MediaType {
    MediaType::Other(JSON_MEDIA_TYPE.to_string())
}

fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(&value)
        .map_err(|e| crate::error!("Failed to encode JSON record `{name}`: {e}"))
}

/// Write `bytes` to the registry's BlobStore and build the in-memory
/// [`RecordRef`]: an OCI layer descriptor carrying the experiment /
/// record annotations, plus the matching blob record for commit-time
/// publication.
fn stage_record_ref(
    registry: &LocalRegistry,
    space: Space,
    run_id: Option<u64>,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<(RecordRef, BlobRecord)> {
    let mut blob = registry.blobs().put_bytes(bytes)?;
    blob.media_type = Some(media_type.to_string());

    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = run_id {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_RECORD_NAME.to_string(), name.to_string());

    let descriptor = build_descriptor(media_type, &blob, annotations)?;
    Ok((
        RecordRef {
            name: name.to_string(),
            descriptor,
        },
        blob,
    ))
}
