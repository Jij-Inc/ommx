//! The public `Experiment` / `Run` handles and their `log_*` API.

use super::model::{RecordRef, RunState, RunStatus, Space, UnsealedExperimentState};
use super::{build_descriptor, commit, ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::{media_types, sha256_digest, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::{str::FromStr, sync::Arc};

/// OCI layer media type for JSON record payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// A mutable, unsealed experiment session. See the [module documentation](super).
#[derive(Debug)]
pub struct Experiment {
    pub(super) registry: Arc<LocalRegistry>,
    session_id: uuid::Uuid,
    active_runs: Arc<AtomicUsize>,
    pub(super) state: UnsealedExperimentState,
}

/// A sealed experiment session whose root artifact manifest has been
/// written and published.
#[derive(Debug, Clone)]
pub struct SealedExperiment {
    artifact: LocalArtifact,
}

/// A handle to a single run within an [`Experiment`].
///
/// A `Run` does not mutably borrow the parent experiment. It writes
/// payload bytes to the registry CAS immediately, keeps run-scoped
/// records / parameters locally, and writes back to the parent
/// experiment only when [`Self::finish`] or [`Self::fail`] consumes the
/// handle. This lets multiple runs be open at once while keeping
/// Experiment state updates at the run lifecycle boundary.
#[derive(Debug)]
pub struct Run {
    registry: Arc<LocalRegistry>,
    session_id: uuid::Uuid,
    active_runs: Arc<AtomicUsize>,
    state: Option<RunState>,
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
            session_id: uuid::Uuid::new_v4(),
            active_runs: Arc::new(AtomicUsize::new(0)),
            state: UnsealedExperimentState {
                name: name.into(),
                requested_ref,
                records: Vec::new(),
                runs: Vec::new(),
                next_run_id: 0,
            },
        }
    }

    /// Start a new [`Run`]. Each run gets a fresh 0-based `run_id`.
    pub fn run(&mut self) -> Result<Run> {
        let run_id = self.state.next_run_id;
        self.state.next_run_id += 1;
        self.active_runs.fetch_add(1, Ordering::SeqCst);
        Ok(Run {
            registry: Arc::clone(&self.registry),
            session_id: self.session_id,
            active_runs: Arc::clone(&self.active_runs),
            state: Some(RunState::new(run_id)),
        })
    }

    /// Record arbitrary bytes with an explicit OCI media type in the
    /// experiment space.
    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_record(name, media_type, bytes.as_ref())
    }

    /// Record a JSON-serialisable value in the experiment space.
    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in the experiment space.
    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Record a [`Solution`] in the experiment space.
    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Record a [`SampleSet`] in the experiment space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    fn add_record(&mut self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let record_ref = store_record_ref(
            &self.registry,
            Space::Experiment,
            None,
            name,
            media_type,
            bytes,
        )?;
        upsert_record_ref(&mut self.state.records, record_ref);
        Ok(())
    }

    fn push_closed_run(&mut self, run: RunState, session_id: uuid::Uuid) -> Result<()> {
        if session_id != self.session_id {
            crate::bail!("Run belongs to a different experiment session");
        }
        if self
            .state
            .runs
            .iter()
            .any(|existing| existing.run_id == run.run_id)
        {
            crate::bail!("Run {} has already been recorded", run.run_id);
        }
        self.state.runs.push(run);
        Ok(())
    }

    /// Seal the session into an immutable OMMX Artifact and publish it
    /// to the Local Registry. Consumes the unsealed session, so further
    /// mutation is impossible in Rust.
    pub fn commit(self) -> Result<SealedExperiment> {
        if self.active_runs.load(Ordering::SeqCst) != 0 {
            crate::bail!("There are still open runs; finish or fail them before committing");
        }
        ensure_no_running_runs(&self.state)?;
        let artifact = commit::commit_experiment_state(&self.registry, &self.state)?;
        Ok(SealedExperiment { artifact })
    }
}

impl RunState {
    fn new(run_id: u64) -> Self {
        Self {
            run_id,
            records: Vec::new(),
            parameters: Default::default(),
            status: RunStatus::Running,
            started_at: Instant::now(),
            elapsed_secs: None,
        }
    }
}

impl SealedExperiment {
    /// The committed artifact handle.
    pub fn artifact(&self) -> LocalArtifact {
        self.artifact.clone()
    }

    /// Consume the sealed experiment and return its artifact handle.
    pub fn into_artifact(self) -> LocalArtifact {
        self.artifact
    }
}

impl Run {
    /// This run's 0-based id within the experiment.
    pub fn run_id(&self) -> u64 {
        self.state_ref().run_id
    }

    /// Record a scalar parameter for this run. Parameters are not
    /// Records: they are materialised at experiment commit time as a
    /// run-parameter table payload used for comparison views.
    pub fn log_parameter(
        &mut self,
        name: impl Into<String>,
        value: impl serde::Serialize,
    ) -> Result<()> {
        let name = name.into();
        let value = serde_json::to_value(value)
            .map_err(|e| crate::error!("Failed to encode run parameter `{name}`: {e}"))?;
        ensure_parameter_scalar(&name, &value)?;
        self.state_mut().parameters.insert(name, value);
        Ok(())
    }

    /// Record arbitrary bytes with an explicit OCI media type in this
    /// run's space.
    pub fn log_record(
        &mut self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.add_record(name, media_type, bytes.as_ref())
    }

    /// Record a JSON-serialisable value in this run's space.
    pub fn log_json(&mut self, name: &str, value: impl serde::Serialize) -> Result<()> {
        let bytes = encode_json(name, &value)?;
        self.log_record(name, json_media_type(), bytes)
    }

    /// Record an [`Instance`] in this run's space.
    pub fn log_instance(&mut self, name: &str, instance: &Instance) -> Result<()> {
        self.log_record(name, media_types::v1_instance(), instance.to_bytes())
    }

    /// Record a [`Solution`] in this run's space.
    pub fn log_solution(&mut self, name: &str, solution: &Solution) -> Result<()> {
        self.log_record(name, media_types::v1_solution(), solution.to_bytes())
    }

    /// Record a [`SampleSet`] in this run's space.
    pub fn log_sample_set(&mut self, name: &str, sample_set: &SampleSet) -> Result<()> {
        self.log_record(name, media_types::v1_sample_set(), sample_set.to_bytes())
    }

    /// Close the run with the `finished` status, record its elapsed
    /// time, and append the closed run state to `experiment`. Consumes
    /// the handle so no further run-scoped data can be added.
    pub fn finish(self, experiment: &mut Experiment) -> Result<()> {
        self.close(experiment, RunStatus::Finished)
    }

    /// Close the run with the `failed` status, record its elapsed time,
    /// and append the closed run state to `experiment`. Consumes the
    /// handle so no further run-scoped data can be added.
    pub fn fail(self, experiment: &mut Experiment) -> Result<()> {
        self.close(experiment, RunStatus::Failed)
    }

    fn state_ref(&self) -> &RunState {
        self.state
            .as_ref()
            .expect("Run state is present until finish/fail consumes the handle")
    }

    fn state_mut(&mut self) -> &mut RunState {
        self.state
            .as_mut()
            .expect("Run state is present until finish/fail consumes the handle")
    }

    fn add_record(&mut self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let run_id = self.state_ref().run_id;
        let record_ref = store_record_ref(
            &self.registry,
            Space::Run,
            Some(run_id),
            name,
            media_type,
            bytes,
        )?;
        upsert_record_ref(&mut self.state_mut().records, record_ref);
        Ok(())
    }

    fn close(mut self, experiment: &mut Experiment, status: RunStatus) -> Result<()> {
        let run_id = self.state_ref().run_id;
        if experiment.session_id != self.session_id {
            crate::bail!("Run belongs to a different experiment session");
        }
        if experiment
            .state
            .runs
            .iter()
            .any(|existing| existing.run_id == run_id)
        {
            crate::bail!("Run {run_id} has already been recorded");
        }

        let mut run = self
            .state
            .take()
            .ok_or_else(|| crate::error!("Run {run_id} was already closed"))?;
        run.elapsed_secs = Some(run.started_at.elapsed().as_secs_f64());
        run.status = status;
        experiment.push_closed_run(run, self.session_id)?;
        self.active_runs.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if self.state.is_some() {
            self.active_runs.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn ensure_parameter_scalar(name: &str, value: &Value) -> Result<()> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
        Value::Array(_) | Value::Object(_) => {
            crate::bail!("Run parameter `{name}` must be a JSON scalar, got {value}")
        }
    }
}

fn ensure_no_running_runs(state: &UnsealedExperimentState) -> Result<()> {
    if let Some(run) = state
        .runs
        .iter()
        .find(|run| run.status == RunStatus::Running)
    {
        crate::bail!(
            "Run {} is still running; finish or fail it before committing the experiment",
            run.run_id
        );
    }
    Ok(())
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
/// record annotations, plus the matching descriptor for commit-time
/// publication.
fn store_record_ref(
    registry: &LocalRegistry,
    space: Space,
    run_id: Option<u64>,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<RecordRef> {
    let digest = sha256_digest(bytes);
    let digest = oci_spec::image::Digest::from_str(&digest)
        .map_err(|e| crate::error!("Failed to parse record blob digest: {e}"))?;
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = run_id {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_RECORD_NAME.to_string(), name.to_string());

    let descriptor = build_descriptor(media_type, &digest, bytes.len() as u64, annotations)?;
    let descriptor = registry.store_blob(descriptor, bytes)?;
    Ok(RecordRef {
        name: name.to_string(),
        descriptor,
    })
}
