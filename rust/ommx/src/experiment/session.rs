//! The public `Experiment` / `Run` handles and their `log_*` API.

use super::model::{RecordRef, RunEntry, RunStatus, Space, UnsealedExperimentState};
use super::{build_descriptor, commit, ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::{media_types, sha256_digest, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// OCI layer media type for JSON record payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// A mutable, unsealed experiment session. See the [module documentation](super).
#[derive(Debug)]
pub struct Experiment {
    pub(super) registry: Arc<LocalRegistry>,
    pub(super) state: Mutex<UnsealedExperimentState>,
}

/// A sealed experiment session whose root artifact manifest has been
/// written and published.
#[derive(Debug, Clone)]
pub struct SealedExperiment {
    artifact: LocalArtifact,
}

/// A handle to a single run within an [`Experiment`].
///
/// A `Run` borrows its parent experiment immutably for `'exp`. It
/// writes payload bytes to the registry CAS immediately, keeps
/// run-scoped records / parameters locally, and writes back to the
/// parent experiment only when [`Self::finish`] or [`Self::fail`]
/// consumes the handle. This lets multiple runs be open at once while
/// Rust prevents committing the parent experiment before live run
/// handles are closed or dropped.
#[derive(Debug)]
pub struct Run<'exp> {
    experiment: &'exp Experiment,
    run_id: u64,
    records: Vec<RecordRef>,
    parameters: BTreeMap<String, Value>,
    started_at: Instant,
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
            state: Mutex::new(UnsealedExperimentState {
                name: name.into(),
                requested_ref,
                records: Vec::new(),
                runs: Vec::new(),
                next_run_id: 0,
            }),
        }
    }

    /// Start a new [`Run`]. Each run gets a fresh 0-based `run_id`.
    pub fn run(&self) -> Result<Run<'_>> {
        let mut state = self.lock_state();
        let run_id = state.next_run_id;
        state.next_run_id += 1;
        Ok(Run {
            experiment: self,
            run_id,
            records: Vec::new(),
            parameters: BTreeMap::new(),
            started_at: Instant::now(),
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
        let record_ref = store_record_ref(
            &self.registry,
            Space::Experiment,
            None,
            name,
            media_type,
            bytes,
        )?;
        let mut state = self.lock_state();
        upsert_record_ref(&mut state.records, record_ref);
        Ok(())
    }

    fn push_closed_run(&self, run: RunEntry) -> Result<()> {
        let mut state = self.lock_state();
        if state
            .runs
            .iter()
            .any(|existing| existing.run_id == run.run_id)
        {
            crate::bail!("Run {} has already been recorded", run.run_id);
        }
        state.runs.push(run);
        Ok(())
    }

    fn lock_state(&self) -> MutexGuard<'_, UnsealedExperimentState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!("Experiment state mutex was poisoned; continuing with inner state");
                poisoned.into_inner()
            }
        }
    }

    /// Seal the session into an immutable OMMX Artifact and publish it
    /// to the Local Registry. Consumes the unsealed session, so further
    /// mutation is impossible in Rust. A live [`Run`] borrows this
    /// experiment, so Rust also prevents committing while a run handle
    /// is still in scope.
    pub fn commit(self) -> Result<SealedExperiment> {
        let state = match self.state.into_inner() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!("Experiment state mutex was poisoned; committing inner state");
                poisoned.into_inner()
            }
        };
        let artifact = commit::commit_experiment_state(&self.registry, state)?;
        Ok(SealedExperiment { artifact })
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

impl<'exp> Run<'exp> {
    /// This run's 0-based id within the experiment.
    pub fn run_id(&self) -> u64 {
        self.run_id
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
        self.parameters.insert(name, value);
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
    /// time, and append the closed run state to the parent experiment.
    /// Consumes the handle so no further run-scoped data can be added.
    pub fn finish(self) -> Result<()> {
        self.close(RunStatus::Finished)
    }

    /// Close the run with the `failed` status, record its elapsed time,
    /// and append the closed run state to the parent experiment.
    /// Consumes the handle so no further run-scoped data can be added.
    pub fn fail(self) -> Result<()> {
        self.close(RunStatus::Failed)
    }

    fn add_record(&mut self, name: &str, media_type: MediaType, bytes: &[u8]) -> Result<()> {
        let record_ref = store_record_ref(
            &self.experiment.registry,
            Space::Run,
            Some(self.run_id),
            name,
            media_type,
            bytes,
        )?;
        upsert_record_ref(&mut self.records, record_ref);
        Ok(())
    }

    fn close(self, status: RunStatus) -> Result<()> {
        let Run {
            experiment,
            run_id,
            records,
            parameters,
            started_at,
        } = self;
        let run = RunEntry {
            run_id,
            records,
            parameters,
            status,
            elapsed_secs: started_at.elapsed().as_secs_f64(),
        };
        experiment.push_closed_run(run)?;
        Ok(())
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
