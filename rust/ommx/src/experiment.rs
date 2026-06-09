//! Experiment / Run session model.
//!
//! An [`Experiment`] is a mutable session that groups a set of named
//! payloads (attachments) — instances, solutions, sample sets, JSON values,
//! or caller-defined media types — together with one or more [`Run`]s.
//! Attachments belong either
//! to the *experiment space* (shared by the whole experiment) or to a
//! *run space* (owned by a single [`Run`]).
//! Run parameters are separate table data: [`Run::log_parameter`] captures
//! bool / int64 / float64 / string scalar values for comparison views,
//! and commit materialises them as a typed column-oriented aggregate
//! run-parameter layer instead of individual Attachments.
//!
//! Each `log_*` call writes its payload to the Local Registry immediately,
//! keeping only
//! [`crate::artifact::local_registry::StoredDescriptor`] values in
//! memory. Until commit, the experiment is unsealed: some or all
//! component blobs may already be stored, but no root manifest has been
//! stored for the whole experiment. [`Experiment::commit`] seals that
//! mutable session into a single immutable OMMX Artifact whose manifest
//! references those already-stored blobs. The registry-level operation
//! that updates the image ref is publish; the Experiment-level
//! operation remains commit.
//!
//! Closing a [`Run`] publishes a best-effort draft checkpoint for the
//! parent Experiment. A successful [`Experiment::commit`] publishes the
//! requested Experiment image reference and removes the local checkpoint
//! when present. Failed or interrupted Python context-manager exits are
//! represented as checkpoint Experiments with `failed` or `interrupted`
//! status; callers resume through the original requested image name
//! rather than through a checkpoint Artifact handle.
//!
//! Forking a sealed Experiment creates a new unsealed child Experiment.
//! The child manifest records the parent manifest as its OCI `subject`,
//! while existing payload blobs remain shared through the Local
//! Registry's content-addressed storage. Local Registry GC treats live
//! refs, checkpoint refs, and traversed subject manifests as roots, so
//! payloads reachable from kept parent Experiments are retained.
//!
//! ```ignore
//! use ommx::artifact::ImageRef;
//! use ommx::experiment::{AttachmentLogger, Experiment, Name};
//!
//! let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/scip_reblock115:latest")?;
//! let exp = Experiment::new(image_name)?;
//! let anonymous_exp = Experiment::new(Name::Anonymous)?;
//! exp.log_json("dataset", serde_json::json!("miplib2017"))?;
//!
//! let mut run = exp.run()?;
//! run.log_parameter("solver", "scip")?;
//! run.log_instance("candidate", &instance, Default::default())?;
//! run.finish()?;
//!
//! let artifact = exp.commit()?.into_artifact();
//! ```
//!
//! The module is split by data terms: `run` contains `Run` lifecycle
//! operations, `attachment` contains Attachment descriptor helpers,
//! `parameter` contains parameter values, run-local parameter sets,
//! and the committed run-parameter table, `config` contains the
//! serialized Experiment structure, `sealed` contains read-only sealed
//! Experiment data reconstructed from committed artifacts, and
//! `artifact` maps the unsealed experiment state onto an OMMX Artifact.

mod artifact;
mod attachment;
pub mod config;
mod dynamic;
mod logging;
mod parameter;
mod run;
mod sealed;

#[cfg(test)]
mod tests;

pub use attachment::{detect_file_media_type, AttachmentTable, DEFAULT_FILE_MEDIA_TYPE};
pub use dynamic::{ExperimentDyn, RunDyn, SealedRunDyn, SolveDyn};
pub use logging::AttachmentLogger;
pub use parameter::{ParameterValue, RunParameterCell};
pub use sealed::{SealedRun, Solve};

use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor, TempLocalRegistry};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use anyhow::{ensure, Context, Result};
use attachment::{read_file_attachment, store_attachment_descriptor};
use oci_spec::image::MediaType;
use parameter::ParameterSet;
use rmpv::Value as MessagePackValue;
use std::sync::{Mutex, MutexGuard};
use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
    path::Path,
};

// --- Artifact mapping constants ---------------------------------------------

const EXPERIMENT_STATUS_FINISHED: &str = "finished";
const EXPERIMENT_STATUS_DRAFT: &str = "draft";
const EXPERIMENT_STATUS_FAILED: &str = "failed";
const EXPERIMENT_STATUS_INTERRUPTED: &str = "interrupted";

const RUN_PARAMETERS_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-parameters+json";
const EXPERIMENT_CONFIG_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.config+json";

const RUN_STATUS_FINISHED: &str = "finished";
const RUN_STATUS_FAILED: &str = "failed";
const RUN_STATUS_INTERRUPTED: &str = "interrupted";

/// Lifecycle status of a sealed Experiment Artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentStatus {
    /// The Experiment was committed successfully.
    Finished,
    /// The Experiment is an uncommitted checkpoint with closed Run state.
    Draft,
    /// The Experiment exited with an exception and retained partial state.
    Failed,
    /// The Experiment was interrupted by the user and retained partial state.
    Interrupted,
}

impl ExperimentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => EXPERIMENT_STATUS_FINISHED,
            Self::Draft => EXPERIMENT_STATUS_DRAFT,
            Self::Failed => EXPERIMENT_STATUS_FAILED,
            Self::Interrupted => EXPERIMENT_STATUS_INTERRUPTED,
        }
    }

    fn from_config(status: &str) -> Result<Self> {
        match status {
            EXPERIMENT_STATUS_FINISHED => Ok(Self::Finished),
            EXPERIMENT_STATUS_DRAFT => Ok(Self::Draft),
            EXPERIMENT_STATUS_FAILED => Ok(Self::Failed),
            EXPERIMENT_STATUS_INTERRUPTED => Ok(Self::Interrupted),
            _ => {
                crate::bail!(
                    "Experiment status is {status}, expected {EXPERIMENT_STATUS_FINISHED}, \
                     {EXPERIMENT_STATUS_DRAFT}, {EXPERIMENT_STATUS_FAILED}, or \
                     {EXPERIMENT_STATUS_INTERRUPTED}"
                )
            }
        }
    }
}

impl std::fmt::Display for ExperimentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status of a closed Run recorded in an Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    /// The Run context exited normally or was explicitly finished.
    Finished,
    /// The Run context exited with an exception and retained partial state.
    Failed,
    /// The Run context was interrupted by the user and retained partial state.
    Interrupted,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => RUN_STATUS_FINISHED,
            Self::Failed => RUN_STATUS_FAILED,
            Self::Interrupted => RUN_STATUS_INTERRUPTED,
        }
    }

    fn from_config(status: &str) -> Result<Self> {
        match status {
            RUN_STATUS_FINISHED => Ok(Self::Finished),
            RUN_STATUS_FAILED => Ok(Self::Failed),
            RUN_STATUS_INTERRUPTED => Ok(Self::Interrupted),
            _ => {
                crate::bail!(
                    "Run status is {status}, expected {RUN_STATUS_FINISHED}, \
                     {RUN_STATUS_FAILED}, or {RUN_STATUS_INTERRUPTED}"
                )
            }
        }
    }
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A mutable, unsealed experiment session. See the [module documentation](self).
#[derive(Debug)]
pub struct Experiment<'reg> {
    registry: &'reg LocalRegistry,
    state: Mutex<UnsealedExperimentState<'reg>>,
}

/// A sealed experiment session whose root artifact manifest has been
/// written and published.
#[derive(Debug, Clone)]
pub struct SealedExperiment<'reg> {
    status: ExperimentStatus,
    artifact: LocalArtifact<'reg>,
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    runs: BTreeMap<u64, sealed::SealedRun<'reg>>,
    run_parameters: parameter::RunParameterTable,
}

/// Opaque Run trace payload.
///
/// The Rust SDK does not decode, validate, or interpret OpenTelemetry
/// spans. `Trace` is a storage boundary type: it marks a byte payload as
/// a Run trace payload, while producers and renderers such as the
/// Python SDK own the concrete OpenTelemetry encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    bytes: Vec<u8>,
}

impl Trace {
    /// Build a trace payload from encoded trace bytes.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Encoded trace bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume the trace and return its encoded bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// User-facing name policy for a new Experiment.
///
/// `Name` is resolved to a concrete [`ImageRef`] when the Experiment
/// is created. The unsealed Experiment state keeps only that resolved
/// image name, so commit always publishes to a concrete Local Registry
/// ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name {
    /// Publish under the caller-provided OCI image reference.
    Named(ImageRef),
    /// Generate a fresh local name of the form
    /// `<registry-id8>.ommx.local/experiment:<timestamp>-<nonce>`.
    Anonymous,
}

impl Name {
    fn resolve(self, registry: &LocalRegistry) -> Result<ImageRef> {
        match self {
            Self::Named(image_name) => Ok(image_name),
            Self::Anonymous => registry.synthesize_anonymous_experiment_image_name(),
        }
    }
}

impl From<ImageRef> for Name {
    fn from(image_name: ImageRef) -> Self {
        Self::Named(image_name)
    }
}

/// A handle to a single run within an [`Experiment`].
///
/// A `Run` borrows its parent experiment immutably for `'exp`. It
/// writes payload bytes to the registry CAS immediately, keeps
/// run-scoped attachments / parameters locally, and writes back to the
/// parent experiment only when [`Self::finish`] consumes the handle.
/// This lets multiple runs be open at once while Rust prevents
/// committing the parent experiment before live run handles are closed
/// or dropped.
#[derive(Debug)]
pub struct Run<'exp, 'reg> {
    experiment: &'exp Experiment<'reg>,
    run_id: u64,
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    trace: Option<StoredDescriptor<'reg>>,
    solves: Vec<SolveEntry<'reg>>,
    next_solve_id: u64,
    parameters: ParameterSet,
}

/// A closed logical Run recorded in an unsealed Experiment.
///
/// `Run<'exp>` is the live handle: it borrows the parent Experiment and
/// accepts run-scoped attachments and parameters. `RunEntry` is the row
/// stored by the Experiment after `Run::finish` consumes that handle.
/// Commit later projects it to aggregate parameter and attachment index
/// layers.
#[derive(Debug)]
struct RunEntry<'reg> {
    run_id: u64,
    status: RunStatus,
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    trace: Option<StoredDescriptor<'reg>>,
    solves: Vec<SolveEntry<'reg>>,
    parameters: ParameterSet,
}

#[derive(Debug, Clone)]
struct SolveEntry<'reg> {
    solve_id: u64,
    input: StoredDescriptor<'reg>,
    output: StoredDescriptor<'reg>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<StoredDescriptor<'reg>>,
}

/// Adapter diagnostics payload for one Solve.
#[derive(Debug, Clone)]
pub struct SolveDiagnosticPayload {
    value: MessagePackValue,
}

impl SolveDiagnosticPayload {
    /// Create a diagnostics payload from MessagePack bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let mut cursor = Cursor::new(&bytes);
        let value = rmpv::decode::read_value(&mut cursor)
            .context("Solve diagnostic payload must be valid MessagePack")?;
        ensure!(
            cursor.position() == bytes.len() as u64,
            "Solve diagnostic payload must contain exactly one MessagePack value",
        );
        Self::from_value(value)
    }

    /// Create a diagnostics payload from a decoded MessagePack value.
    pub fn from_value(value: MessagePackValue) -> Result<Self> {
        ensure!(
            matches!(value, MessagePackValue::Array(_)),
            "Solve diagnostic payload must decode to a MessagePack array",
        );
        Ok(Self { value })
    }

    /// Decoded MessagePack value. The top-level value is always an array.
    pub fn value(&self) -> &MessagePackValue {
        &self.value
    }

    pub(crate) fn to_msgpack_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &self.value)
            .context("Failed to encode Solve diagnostic payload as MessagePack")?;
        Ok(bytes)
    }
}

fn read_solve_diagnostic_payload(
    solve_id: u64,
    descriptor: &StoredDescriptor<'_>,
) -> Result<(Vec<u8>, SolveDiagnosticPayload)> {
    descriptor.ensure_media_type(&media_types::diagnostic_msgpack())?;
    let bytes = descriptor.registry().get_blob(descriptor)?;
    let payload = SolveDiagnosticPayload::new(bytes.clone())
        .with_context(|| format!("Invalid Solve {solve_id} diagnostic payload"))?;
    Ok((bytes, payload))
}

/// Mutable experiment state before the root manifest is sealed. A live
/// [`Run`] borrows the parent experiment while it adds run-scoped
/// attachments. Closed runs are stored as [`RunEntry`] values.
#[derive(Debug)]
struct UnsealedExperimentState<'reg> {
    /// Image name the committed Experiment artifact is published
    /// under. Experiment identity is the Local Registry ref; there is
    /// no separate experiment-name field in the artifact model.
    image_name: ImageRef,
    /// Parent Experiment manifest descriptor for lineage. `None` for
    /// a root Experiment and `Some` for a forked child Experiment.
    subject: Option<oci_spec::image::Descriptor>,
    /// Experiment-space attachments.
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    runs: BTreeMap<u64, RunEntry<'reg>>,
    next_run_id: u64,
}

impl Experiment<'static> {
    /// Start a new experiment session backed by the user's default
    /// Local Registry. The committed artifact is published under the
    /// resolved `name`.
    pub fn new(name: impl Into<Name>) -> Result<Self> {
        let registry = LocalRegistry::shared_default()?;
        Self::with_registry(registry, name)
    }
}

impl<'reg> Experiment<'reg> {
    /// Create a temporary Local Registry, run an experiment callback
    /// against it, and delete the registry when the callback returns.
    ///
    /// This is intended for Rust SDK tests that need an isolated
    /// registry while still exercising the same Local Registry-backed
    /// artifact path as production code.
    pub fn with_temp_local_registry<T>(
        name: impl Into<Name>,
        f: impl FnOnce(Experiment<'_>) -> anyhow::Result<T>,
    ) -> Result<T> {
        let temp = TempLocalRegistry::new()?;
        let experiment = Experiment::with_registry(temp.registry(), name)?;
        f(experiment)
    }

    /// Start a new experiment session against an explicit Local
    /// Registry. The committed artifact is published under the
    /// resolved `name`.
    pub fn with_registry(registry: &'reg LocalRegistry, name: impl Into<Name>) -> Result<Self> {
        let image_name = name.into().resolve(registry)?;
        Ok(Experiment {
            registry,
            state: Mutex::new(UnsealedExperimentState {
                image_name,
                subject: None,
                attachments: AttachmentTable::new(),
                runs: BTreeMap::new(),
                next_run_id: 0,
            }),
        })
    }

    /// Concrete Local Registry image name this Experiment will publish
    /// to when committed.
    pub fn image_name(&self) -> ImageRef {
        self.lock_state().image_name.clone()
    }

    /// Start a new [`Run`]. Each run gets a fresh 0-based `run_id`.
    pub fn run(&self) -> Result<Run<'_, 'reg>> {
        let mut state = self.lock_state();
        let run_id = allocate_next_run_id(&mut state.next_run_id)?;
        Ok(Run {
            experiment: self,
            run_id,
            attachments: AttachmentTable::new(),
            trace: None,
            solves: Vec::new(),
            next_solve_id: 0,
            parameters: ParameterSet::new(),
        })
    }

    fn push_closed_run(&self, run: RunEntry<'reg>) -> Result<()> {
        let mut state = self.lock_state();
        if state.runs.contains_key(&run.run_id) {
            crate::bail!("Run {} has already been registered", run.run_id);
        }
        state.runs.insert(run.run_id, run);
        if let Err(error) = state.autosave_checkpoint(self.registry) {
            tracing::warn!(
                error = %error,
                "Failed to publish Experiment autosave checkpoint after Run close"
            );
        }
        Ok(())
    }

    fn lock_state(&self) -> MutexGuard<'_, UnsealedExperimentState<'reg>> {
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
    pub fn commit(self) -> Result<SealedExperiment<'reg>> {
        let state = match self.state.into_inner() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!("Experiment state mutex was poisoned; committing inner state");
                poisoned.into_inner()
            }
        };
        let artifact = state.commit(self.registry)?;
        SealedExperiment::from_artifact(artifact)
    }
}

impl<'reg> AttachmentLogger for &Experiment<'reg> {
    fn log_attachment(
        self,
        name: &str,
        media_type: MediaType,
        bytes: impl AsRef<[u8]>,
        annotations: HashMap<String, String>,
    ) -> Result<()> {
        {
            let state = self.lock_state();
            if state.attachments.contains_key(name) {
                crate::bail!("Attachment `{name}` already exists");
            }
        }
        let descriptor =
            store_attachment_descriptor(self.registry, media_type, bytes.as_ref(), annotations)?;
        self.lock_state()
            .attachments
            .insert(name.to_string(), descriptor, None)
            .with_context(|| format!("Failed to register attachment `{name}`"))?;
        Ok(())
    }

    fn log_file(
        self,
        name: &str,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        filename: Option<&str>,
    ) -> Result<()> {
        let (media_type, bytes, filename) = read_file_attachment(path, media_type, filename)?;
        {
            let state = self.lock_state();
            if state.attachments.contains_key(name) {
                crate::bail!("Attachment `{name}` already exists");
            }
        }
        let descriptor =
            store_attachment_descriptor(self.registry, media_type, bytes.as_ref(), HashMap::new())?;
        self.lock_state()
            .attachments
            .insert(name.to_string(), descriptor, Some(filename))
            .with_context(|| format!("Failed to register attachment `{name}`"))?;
        Ok(())
    }
}

impl<'reg> SealedExperiment<'reg> {
    /// The committed artifact handle.
    pub fn artifact(&self) -> LocalArtifact<'reg> {
        self.artifact.clone()
    }

    /// Consume the sealed experiment and return its artifact handle.
    pub fn into_artifact(self) -> LocalArtifact<'reg> {
        self.artifact
    }

    /// Fork this sealed Experiment into a new unsealed child Experiment.
    ///
    /// The parent Experiment is not modified. Existing experiment
    /// attachments, runs, solves, and run parameters are carried into
    /// the child state, while the committed child Artifact records the
    /// parent manifest descriptor as its OCI `subject`.
    pub fn fork(&self, name: impl Into<Name>) -> Result<Experiment<'reg>> {
        let registry = self.artifact.registry();
        let image_name = name.into().resolve(registry)?;
        let subject = Some(self.artifact.stored_manifest_descriptor()?.into());
        let mut runs = BTreeMap::new();
        let mut parameters_by_run = self.run_parameters.parameter_sets()?;

        for run in self.runs.values() {
            let parameters = parameters_by_run
                .remove(&run.run_id())
                .unwrap_or_else(ParameterSet::new);
            let solves = run
                .solves()
                .iter()
                .map(|solve| SolveEntry {
                    solve_id: solve.solve_id(),
                    input: solve.input_descriptor().clone(),
                    output: solve.output_descriptor().clone(),
                    adapter: solve.adapter().to_string(),
                    adapter_options: solve.adapter_options().to_string(),
                    diagnostics: solve.diagnostic_descriptor().cloned(),
                })
                .collect();
            runs.insert(
                run.run_id(),
                RunEntry {
                    run_id: run.run_id(),
                    status: run.status().clone(),
                    attachments: run.attachment_table().clone(),
                    trace: run.trace_descriptor().cloned(),
                    solves,
                    parameters,
                },
            );
        }

        Ok(Experiment {
            registry,
            state: Mutex::new(UnsealedExperimentState {
                image_name,
                subject,
                attachments: self.attachments.clone(),
                next_run_id: next_run_id(runs.keys().copied())?,
                runs,
            }),
        })
    }
}

fn next_run_id(run_ids: impl Iterator<Item = u64>) -> Result<u64> {
    match run_ids.max() {
        Some(max) => max
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Run ID space is exhausted")),
        None => Ok(0),
    }
}

fn allocate_next_run_id(next_run_id: &mut u64) -> Result<u64> {
    let run_id = *next_run_id;
    *next_run_id = next_run_id
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("Run ID space is exhausted"))?;
    Ok(run_id)
}
