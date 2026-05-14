//! Experiment / Run session model.
//!
//! An [`Experiment`] is a mutable session that groups a set of named
//! payloads ([`Record`]s) — instances, solutions, sample sets, JSON
//! metadata — together with one or more [`Run`]s. Records belong either
//! to the *experiment space* (shared by the whole experiment) or to a
//! *run space* (owned by a single [`Run`]).
//!
//! Each `log_*` call writes its payload to the Local Registry's
//! content-addressed BlobStore immediately, keeping only an OCI
//! descriptor in memory. [`Experiment::commit`] then seals the session
//! into a single immutable OMMX Artifact whose manifest references
//! those already-stored blobs.
//!
//! ```ignore
//! use ommx::experiment::Experiment;
//!
//! let exp = Experiment::new("scip_reblock115")?;
//! exp.log_metadata("dataset", serde_json::json!("miplib2017"))?;
//!
//! let run = exp.run()?;
//! run.log_instance("candidate", &instance)?;
//! run.finish()?;
//!
//! let artifact = exp.commit()?;
//! ```

use crate::artifact::local_registry::{
    BlobRecord, LocalRegistry, RefConflictPolicy, RefUpdate, BLOB_KIND_CONFIG,
};
use crate::artifact::{media_types, sha256_digest, stable_json_bytes, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, ImageManifestBuilder, MediaType};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

// --- Artifact mapping constants ---------------------------------------------

const ARTIFACT_KIND_EXPERIMENT: &str = "experiment";
const EXPERIMENT_SCHEMA_V1: &str = "v1";
const EXPERIMENT_STATUS_FINISHED: &str = "finished";

const ANN_ARTIFACT_KIND: &str = "org.ommx.artifact.kind";
const ANN_EXPERIMENT_SCHEMA: &str = "org.ommx.experiment.schema";
const ANN_EXPERIMENT_NAME: &str = "org.ommx.experiment.name";
const ANN_EXPERIMENT_STATUS: &str = "org.ommx.experiment.status";
const ANN_SPACE: &str = "org.ommx.experiment.space";
const ANN_RUN_ID: &str = "org.ommx.experiment.run_id";
const ANN_LAYER: &str = "org.ommx.experiment.layer";
const ANN_RECORD_KIND: &str = "org.ommx.record.kind";
const ANN_RECORD_NAME: &str = "org.ommx.record.name";

const JSON_MEDIA_TYPE: &str = "application/json";
const EXPERIMENT_INDEX_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment+json";
const RUN_ATTRIBUTES_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-attributes+json";
const LAYER_KIND_INDEX: &str = "index";
const LAYER_KIND_RUN_ATTRIBUTES: &str = "run-attributes";

// --- Domain enums -----------------------------------------------------------

/// The storage space a [`Record`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single [`Run`].
    Run,
}

impl Space {
    fn as_str(self) -> &'static str {
        match self {
            Space::Experiment => "experiment",
            Space::Run => "run",
        }
    }
}

/// The kind of payload a [`Record`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordKind {
    /// Small JSON context (dataset name, source problem id, ...).
    Metadata,
    /// JSON-serialisable structured object.
    Object,
    /// An [`crate::Instance`].
    Instance,
    /// A [`crate::Solution`].
    Solution,
    /// A [`crate::SampleSet`].
    SampleSet,
}

impl RecordKind {
    fn as_str(self) -> &'static str {
        match self {
            RecordKind::Metadata => "metadata",
            RecordKind::Object => "object",
            RecordKind::Instance => "instance",
            RecordKind::Solution => "solution",
            RecordKind::SampleSet => "sampleset",
        }
    }
}

/// Lifecycle status of a [`Run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunStatus {
    /// The run is open and still accepting records.
    Running,
    /// The run finished normally.
    Finished,
    /// The run ended via a failure.
    Failed,
}

impl RunStatus {
    fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Failed => "failed",
        }
    }
}

// --- In-memory state --------------------------------------------------------

/// A named payload that has already been written to the BlobStore.
#[derive(Debug, Clone)]
struct Record {
    kind: RecordKind,
    name: String,
    /// OCI layer descriptor; carries the experiment / record annotations.
    descriptor: Descriptor,
    /// IndexStore blob record for the CAS-written payload.
    blob: BlobRecord,
}

#[derive(Debug)]
struct RunState {
    run_id: u64,
    records: Vec<Record>,
    status: RunStatus,
    started_at: Instant,
    elapsed_secs: Option<f64>,
}

#[derive(Debug)]
struct ExperimentState {
    name: String,
    /// Image name the committed artifact is published under. `None`
    /// means an anonymous name is synthesised at commit time.
    requested_ref: Option<ImageRef>,
    /// Experiment-space records.
    records: Vec<Record>,
    runs: Vec<RunState>,
    next_run_id: u64,
    committed: bool,
    artifact: Option<LocalArtifact>,
}

// --- Public handles ---------------------------------------------------------

/// A mutable experiment session. See the [module documentation](self).
#[derive(Debug, Clone)]
pub struct Experiment {
    registry: Arc<LocalRegistry>,
    state: Arc<Mutex<ExperimentState>>,
}

/// A handle to a single run within an [`Experiment`].
///
/// `Run` shares the parent experiment's state: `log_*` calls add records
/// to the run space, and [`Run::finish`] / [`Run::fail`] close the run's
/// lifecycle. Cloning a `Run` yields another handle to the same run.
#[derive(Debug, Clone)]
pub struct Run {
    registry: Arc<LocalRegistry>,
    state: Arc<Mutex<ExperimentState>>,
    run_id: u64,
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
        let artifact = build_and_publish(&self.registry, &state)?;
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

    /// Close the run with [`RunStatus::Finished`] and record its elapsed
    /// time. A no-op if the run is already closed.
    pub fn finish(&self) -> Result<()> {
        self.close(RunStatus::Finished)
    }

    /// Close the run with [`RunStatus::Failed`] and record its elapsed
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

// --- Free helpers -----------------------------------------------------------

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
/// annotations, plus the matching [`BlobRecord`].
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

fn build_descriptor(
    media_type: MediaType,
    blob: &BlobRecord,
    annotations: HashMap<String, String>,
) -> Result<Descriptor> {
    let digest = Digest::from_str(&blob.digest)
        .map_err(|e| crate::error!("Failed to parse blob digest `{}`: {e}", blob.digest))?;
    DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest)
        .size(blob.size)
        .annotations(annotations)
        .build()
        .map_err(|e| crate::error!("Failed to build OCI descriptor: {e}"))
}

/// CAS-write a commit-time aggregate JSON layer and return its
/// descriptor (with the `org.ommx.experiment.layer` annotation) plus
/// blob record.
fn stage_aggregate_layer(
    registry: &LocalRegistry,
    media_type: &str,
    layer_kind: &str,
    bytes: &[u8],
) -> Result<(Descriptor, BlobRecord)> {
    let mut blob = registry.blobs().put_bytes(bytes)?;
    blob.media_type = Some(media_type.to_string());
    let mut annotations = HashMap::new();
    annotations.insert(ANN_LAYER.to_string(), layer_kind.to_string());
    let descriptor =
        build_descriptor(MediaType::Other(media_type.to_string()), &blob, annotations)?;
    Ok((descriptor, blob))
}

/// Assemble the experiment manifest from the already-staged record
/// blobs plus the commit-time aggregate layers, and publish it.
fn build_and_publish(
    registry: &Arc<LocalRegistry>,
    state: &ExperimentState,
) -> Result<LocalArtifact> {
    let mut layers: Vec<Descriptor> = Vec::new();
    let mut blob_records: Vec<BlobRecord> = Vec::new();
    let mut seen_digests: HashSet<String> = HashSet::new();

    // Record layers: experiment space first, then each run's space.
    // `layers[]` keeps one descriptor per record (digests may repeat
    // across annotation-distinct layers); `blob_records` is de-duped
    // by digest, since the BlobStore shares identical payloads.
    let run_records = state.runs.iter().flat_map(|run| run.records.iter());
    for record in state.records.iter().chain(run_records) {
        layers.push(record.descriptor.clone());
        if seen_digests.insert(record.blob.digest.clone()) {
            blob_records.push(record.blob.clone());
        }
    }

    // Aggregate layers, materialised at commit time.
    let run_attributes = serde_json::to_vec(&run_attributes_json(state))
        .map_err(|e| crate::error!("Failed to encode run attributes JSON: {e}"))?;
    let (descriptor, blob) = stage_aggregate_layer(
        registry,
        RUN_ATTRIBUTES_MEDIA_TYPE,
        LAYER_KIND_RUN_ATTRIBUTES,
        &run_attributes,
    )?;
    layers.push(descriptor);
    if seen_digests.insert(blob.digest.clone()) {
        blob_records.push(blob);
    }

    let index = serde_json::to_vec(&experiment_index_json(state))
        .map_err(|e| crate::error!("Failed to encode experiment index JSON: {e}"))?;
    let (descriptor, blob) = stage_aggregate_layer(
        registry,
        EXPERIMENT_INDEX_MEDIA_TYPE,
        LAYER_KIND_INDEX,
        &index,
    )?;
    layers.push(descriptor);
    if seen_digests.insert(blob.digest.clone()) {
        blob_records.push(blob);
    }

    // OCI 1.1 empty config blob. Built without an `annotations` field
    // to match `LocalArtifactBuilder::stage`.
    let empty_config_bytes = media_types::OCI_EMPTY_CONFIG_BYTES.to_vec();
    let config_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::EmptyJSON)
        .digest(
            Digest::from_str(&sha256_digest(&empty_config_bytes))
                .map_err(|e| crate::error!("Failed to parse empty config digest: {e}"))?,
        )
        .size(empty_config_bytes.len() as u64)
        .build()
        .map_err(|e| crate::error!("Failed to build empty config descriptor: {e}"))?;
    let mut config_blob = registry.blobs().put_bytes(&empty_config_bytes)?;
    config_blob.media_type = Some(MediaType::EmptyJSON.to_string());
    config_blob.kind = BLOB_KIND_CONFIG.to_string();
    if seen_digests.insert(config_blob.digest.clone()) {
        blob_records.push(config_blob);
    }

    let manifest = ImageManifestBuilder::default()
        .schema_version(2u32)
        .artifact_type(MediaType::Other(
            media_types::V1_ARTIFACT_MEDIA_TYPE.to_string(),
        ))
        .config(config_descriptor)
        .layers(layers)
        .annotations(manifest_annotations(state))
        .build()
        .map_err(|e| crate::error!("Failed to build experiment OCI image manifest: {e}"))?;
    let manifest_bytes = stable_json_bytes(&manifest)?;
    let manifest_descriptor = DescriptorBuilder::default()
        .media_type(MediaType::ImageManifest)
        .digest(
            Digest::from_str(&sha256_digest(&manifest_bytes))
                .map_err(|e| crate::error!("Failed to parse manifest digest: {e}"))?,
        )
        .size(manifest_bytes.len() as u64)
        .build()
        .map_err(|e| crate::error!("Failed to build manifest descriptor: {e}"))?;

    let image_name = match &state.requested_ref {
        Some(image_ref) => image_ref.clone(),
        None => registry.synthesize_anonymous_image_name()?,
    };

    let ref_update = registry.publish_prestaged_artifact_manifest(
        &image_name,
        &manifest,
        &manifest_descriptor,
        &manifest_bytes,
        &blob_records,
        RefConflictPolicy::KeepExisting,
    )?;
    if let RefUpdate::Conflicted {
        existing_manifest_digest,
        incoming_manifest_digest,
    } = ref_update
    {
        crate::bail!(
            "Local registry ref {image_name} already points to {existing_manifest_digest}; \
             experiment manifest {incoming_manifest_digest} was not published"
        );
    }

    Ok(LocalArtifact::from_parts(
        Arc::clone(registry),
        image_name,
        manifest_descriptor.digest().to_string(),
    ))
}

fn manifest_annotations(state: &ExperimentState) -> HashMap<String, String> {
    HashMap::from([
        (
            ANN_ARTIFACT_KIND.to_string(),
            ARTIFACT_KIND_EXPERIMENT.to_string(),
        ),
        (
            ANN_EXPERIMENT_SCHEMA.to_string(),
            EXPERIMENT_SCHEMA_V1.to_string(),
        ),
        (ANN_EXPERIMENT_NAME.to_string(), state.name.clone()),
        (
            ANN_EXPERIMENT_STATUS.to_string(),
            EXPERIMENT_STATUS_FINISHED.to_string(),
        ),
    ])
}

fn run_attributes_json(state: &ExperimentState) -> serde_json::Value {
    json!({
        "runs": state
            .runs
            .iter()
            .map(|run| json!({
                "run_id": run.run_id,
                "status": run.status.as_str(),
                "elapsed_seconds": run.elapsed_secs,
            }))
            .collect::<Vec<_>>(),
    })
}

fn experiment_index_json(state: &ExperimentState) -> serde_json::Value {
    json!({
        "schema": EXPERIMENT_SCHEMA_V1,
        "name": state.name,
        "experiment_records": state
            .records
            .iter()
            .map(record_index_entry)
            .collect::<Vec<_>>(),
        "runs": state
            .runs
            .iter()
            .map(|run| json!({
                "run_id": run.run_id,
                "records": run.records.iter().map(record_index_entry).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    })
}

fn record_index_entry(record: &Record) -> serde_json::Value {
    json!({
        "kind": record.kind.as_str(),
        "name": record.name,
        "media_type": record.descriptor.media_type().to_string(),
        "digest": record.descriptor.digest().to_string(),
        "size": record.descriptor.size(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A fresh experiment backed by a throwaway temp Local Registry. The
    /// returned `TempDir` must outlive the experiment.
    fn temp_experiment(name: &str) -> (TempDir, Experiment) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let registry = Arc::new(LocalRegistry::open(dir.path()).expect("open temp registry"));
        let experiment = Experiment::with_registry(name, registry, None);
        (dir, experiment)
    }

    fn layer_annotation(layer: &Descriptor, key: &str) -> Option<String> {
        layer
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(key).cloned())
    }

    /// Find the single layer whose `annotations[key]` equals `value`.
    fn find_layer<'a>(layers: &'a [Descriptor], key: &str, value: &str) -> &'a Descriptor {
        let matches: Vec<&Descriptor> = layers
            .iter()
            .filter(|layer| layer_annotation(layer, key).as_deref() == Some(value))
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "expected exactly one layer with {key}={value}"
        );
        matches[0]
    }

    /// `run()` hands out fresh 0-based ids; `finish()` / `fail()` record
    /// the final status and elapsed time, and re-closing is a no-op.
    #[test]
    fn run_lifecycle_assigns_ids_and_records_status() {
        let (_dir, experiment) = temp_experiment("lifecycle");
        let run0 = experiment.run().unwrap();
        let run1 = experiment.run().unwrap();
        assert_eq!(run0.run_id(), 0);
        assert_eq!(run1.run_id(), 1);

        run0.finish().unwrap();
        run1.fail().unwrap();
        run0.finish().unwrap(); // already closed: no-op

        let state = experiment.state.lock().unwrap();
        assert_eq!(state.runs[0].status, RunStatus::Finished);
        assert!(state.runs[0].elapsed_secs.is_some());
        assert_eq!(state.runs[1].status, RunStatus::Failed);
        assert!(state.runs[1].elapsed_secs.is_some());
    }

    /// `log_*` writes the payload to the BlobStore immediately, before
    /// any commit advances a public ref.
    #[test]
    fn log_writes_blob_to_blobstore_immediately() {
        let (_dir, experiment) = temp_experiment("eager-write");
        let run = experiment.run().unwrap();
        run.log_metadata("solver", json!("scip")).unwrap();

        let digest = {
            let state = experiment.state.lock().unwrap();
            assert_eq!(state.runs[0].records.len(), 1);
            state.runs[0].records[0].blob.digest.clone()
        };
        assert!(experiment.registry.blobs().exists(&digest).unwrap());
        assert!(!experiment.is_committed().unwrap());
    }

    /// Logging the same `(space, kind, name)` again replaces the record.
    #[test]
    fn log_upserts_same_space_kind_name() {
        let (_dir, experiment) = temp_experiment("upsert");
        experiment
            .log_metadata("dataset", json!("miplib2017"))
            .unwrap();
        experiment.log_metadata("dataset", json!("qplib")).unwrap();

        let state = experiment.state.lock().unwrap();
        assert_eq!(state.records.len(), 1);
        let bytes = experiment
            .registry
            .blobs()
            .read_bytes(&state.records[0].blob.digest)
            .unwrap();
        assert_eq!(bytes, serde_json::to_vec(&json!("qplib")).unwrap());
    }

    /// `commit()` seals the session into an OMMX Artifact whose manifest
    /// and layer annotations describe the experiment / run records.
    #[test]
    fn commit_produces_experiment_artifact() {
        let (_dir, experiment) = temp_experiment("commit");
        experiment
            .log_metadata("dataset", json!("miplib2017"))
            .unwrap();

        let instance: Instance =
            crate::random::random_deterministic(crate::InstanceParameters::default_lp());
        let run = experiment.run().unwrap();
        run.log_instance("candidate", &instance).unwrap();
        run.log_object("config", json!({ "relaxed": true }))
            .unwrap();
        run.finish().unwrap();

        let artifact = experiment.commit().unwrap();

        let annotations = artifact.annotations().unwrap();
        assert_eq!(
            annotations.get(ANN_ARTIFACT_KIND).map(String::as_str),
            Some(ARTIFACT_KIND_EXPERIMENT)
        );
        assert_eq!(
            annotations.get(ANN_EXPERIMENT_SCHEMA).map(String::as_str),
            Some(EXPERIMENT_SCHEMA_V1)
        );
        assert_eq!(
            annotations.get(ANN_EXPERIMENT_NAME).map(String::as_str),
            Some("commit")
        );
        assert_eq!(
            annotations.get(ANN_EXPERIMENT_STATUS).map(String::as_str),
            Some(EXPERIMENT_STATUS_FINISHED)
        );

        // 3 records (1 experiment-space + 2 run-space) + run-attributes + index.
        let layers = artifact.layers().unwrap();
        assert_eq!(layers.len(), 5);

        let dataset = find_layer(&layers, ANN_RECORD_NAME, "dataset");
        assert_eq!(
            layer_annotation(dataset, ANN_SPACE).as_deref(),
            Some("experiment")
        );
        assert_eq!(
            layer_annotation(dataset, ANN_RECORD_KIND).as_deref(),
            Some("metadata")
        );
        assert!(layer_annotation(dataset, ANN_RUN_ID).is_none());

        let candidate = find_layer(&layers, ANN_RECORD_NAME, "candidate");
        assert_eq!(
            layer_annotation(candidate, ANN_SPACE).as_deref(),
            Some("run")
        );
        assert_eq!(
            layer_annotation(candidate, ANN_RUN_ID).as_deref(),
            Some("0")
        );
        assert_eq!(
            layer_annotation(candidate, ANN_RECORD_KIND).as_deref(),
            Some("instance")
        );
        assert_eq!(candidate.media_type(), &media_types::v1_instance());
        assert_eq!(
            artifact.get_blob(candidate.digest().as_ref()).unwrap(),
            instance.to_bytes()
        );

        // Aggregate layers are not tagged as records.
        let run_attrs = find_layer(&layers, ANN_LAYER, LAYER_KIND_RUN_ATTRIBUTES);
        assert!(layer_annotation(run_attrs, ANN_SPACE).is_none());
        let index = find_layer(&layers, ANN_LAYER, LAYER_KIND_INDEX);
        assert!(layer_annotation(index, ANN_SPACE).is_none());

        // Config is the OCI 1.1 empty config.
        assert_eq!(
            artifact.get_manifest().unwrap().config().media_type(),
            &MediaType::EmptyJSON
        );
    }

    /// After `commit()` the session is sealed: further `log_*` / `run()`
    /// calls — including via a previously obtained `Run` — are errors.
    #[test]
    fn mutation_after_commit_is_rejected() {
        let (_dir, experiment) = temp_experiment("sealed");
        let run = experiment.run().unwrap();
        run.log_metadata("seed", json!(0)).unwrap();
        run.finish().unwrap();
        experiment.commit().unwrap();

        assert!(experiment.log_metadata("late", json!(1)).is_err());
        assert!(experiment.run().is_err());
        assert!(run.log_metadata("late", json!(1)).is_err());
    }

    /// `commit()` is idempotent: the second call returns the artifact
    /// produced by the first.
    #[test]
    fn commit_is_idempotent() {
        let (_dir, experiment) = temp_experiment("idempotent");
        experiment
            .log_metadata("dataset", json!("miplib2017"))
            .unwrap();
        let first = experiment.commit().unwrap();
        let second = experiment.commit().unwrap();
        assert_eq!(first.manifest_digest(), second.manifest_digest());
        assert_eq!(first.image_name(), second.image_name());
    }

    /// A byte-identical record logged by two runs yields two annotation-
    /// distinct layer descriptors backed by one shared CAS blob.
    #[test]
    fn byte_identical_record_across_runs_shares_one_blob() {
        let (_dir, experiment) = temp_experiment("shared-blob");
        let payload = json!({ "formulation": "relaxed" });

        let run0 = experiment.run().unwrap();
        run0.log_object("candidate", payload.clone()).unwrap();
        run0.finish().unwrap();

        let run1 = experiment.run().unwrap();
        run1.log_object("candidate", payload.clone()).unwrap();
        run1.finish().unwrap();

        let artifact = experiment.commit().unwrap();
        let layers = artifact.layers().unwrap();

        let candidates: Vec<&Descriptor> = layers
            .iter()
            .filter(|layer| {
                layer_annotation(layer, ANN_RECORD_NAME).as_deref() == Some("candidate")
            })
            .collect();
        assert_eq!(candidates.len(), 2);
        let mut run_ids: Vec<Option<String>> = candidates
            .iter()
            .map(|layer| layer_annotation(layer, ANN_RUN_ID))
            .collect();
        run_ids.sort();
        assert_eq!(run_ids, vec![Some("0".to_string()), Some("1".to_string())]);
        // Same content -> same digest -> one physical blob.
        assert_eq!(
            candidates[0].digest().to_string(),
            candidates[1].digest().to_string()
        );
    }
}
