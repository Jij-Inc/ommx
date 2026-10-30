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
//! parent Experiment by default. [`Experiment::set_autosave_policy`] can
//! batch, rate-limit, or disable these Run-close checkpoints for sessions
//! with many Runs. A successful [`Experiment::commit`] publishes the
//! requested Experiment image reference and removes the local checkpoint
//! when present. Failed or interrupted Python context-manager exits are
//! represented as checkpoint Experiments with `failed` or `interrupted`
//! status; callers resume through the original requested image name
//! rather than through a checkpoint Artifact handle.
//!
//! Rust callers can use [`Experiment::scoped`] and
//! [`Experiment::scoped_run`] for the same success / failure / interruption
//! transitions while retaining compile-time registry and Run lifetimes.
//! [`Run::interrupt_on_drop`], [`ExperimentDyn::interrupt_on_drop`], and
//! [`RunDyn::interrupt_on_drop`] provide opt-in, best-effort fallbacks for
//! manually managed handles. These operations never push an Artifact to a
//! remote registry.
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
//! run.log_instance("candidate", &instance)?;
//! run.finish()?;
//!
//! let artifact = exp.commit()?.into_artifact();
//! ```
//!
//! A lifecycle-safe scope keeps the caller error while checkpointing partial
//! state:
//!
//! ```ignore
//! use ommx::experiment::{Experiment, Name};
//!
//! let experiment = Experiment::scoped(Name::Anonymous, |experiment| {
//!     experiment.scoped_run(|run| {
//!         run.log_parameter("seed", 1_i64)?;
//!         Ok(())
//!     })?;
//!     Ok(())
//! })?;
//! ```
//!
//! # Choosing an Experiment or Run type
//!
//! The API has two independent axes: whether Rust borrows the Local Registry
//! through lifetimes or owns it through a runtime handle, and whether the
//! domain object is still mutable or has been sealed. The types are separate
//! so Rust callers can choose compile-time ownership guarantees without making
//! dynamic runtimes such as Python model Rust lifetimes.
//! Most Rust SDK code should start with [`Experiment`] and [`Run`]. Choose the
//! `Dyn` family only when the registry owner or mutable handle must cross a
//! boundary that cannot carry Rust lifetimes; choose a sealed type only after
//! mutation has ended. These are representation and lifecycle choices, not
//! additional workflow layers.
//!
//! | Type | Role | Why it is separate |
//! | --- | --- | --- |
//! | [`Experiment`] | Mutable Experiment borrowing a [`LocalRegistry`] | Its registry lifetime and consuming [`Experiment::commit`] make ownership and sealing explicit at compile time. |
//! | [`SealedExperiment`] | Immutable committed Experiment borrowing its registry | It cannot expose mutable Experiment operations; [`SealedExperiment::fork`] creates a new mutable child. |
//! | [`Run`] | Mutable Run borrowing its parent [`Experiment`] | The parent cannot be consumed while a Run borrow is live, and `finish*` writes the closed Run back to that parent. Scoped Experiments opt unresolved Runs into interrupted write-back. |
//! | [`SealedRun`] | Immutable closed Run reconstructed from an Artifact | It reads persisted attachments, Solve records, Sampling records, and trace data without a mutable parent. |
//! | [`Solve`] / [`Sampling`] | Immutable Solve or Sampling record borrowing its registry | Child records use the same registry lifetime as the [`SealedRun`] that owns them. |
//! | [`ExperimentDyn`] | Runtime-owned Experiment used when Rust lifetimes cannot cross the caller boundary | One handle transitions from unsealed to sealed or failed in place, so there is intentionally no `SealedExperimentDyn`. |
//! | [`RunDyn`] | Runtime-owned mutable Run | It keeps the parent Experiment state alive at runtime and consumes itself on `finish*`. |
//! | [`SealedRunDyn`] | Runtime-owned immutable closed Run view | A closed Run has no write-back authority and may outlive the call that returned it. |
//! | [`SolveDyn`] / [`SamplingDyn`] | Runtime-owned immutable Solve or Sampling record | Child records retain the registry owner needed to decode their input, output, and diagnostics. |
//!
//! The borrowed and runtime-owned families model the same Experiment / Run
//! semantics, but intentionally use different representations:
//! [`StoredDescriptor`]
//! values carry a borrowed registry proof, while dynamic handles retain a
//! registry owner and validate raw OCI descriptors at runtime.
//! [`ExperimentLifecycle`] and [`RunLifecycle`] are the complete lifecycle
//! values; [`ExperimentStatus`] and [`RunStatus`] are their reason-free
//! discriminants used by listings and compatibility accessors.
//! [`SolveStatus`] and [`SamplingStatus`] remain status-only values.
//! The [`config`] types describe the serialized Artifact schema and are not
//! runtime Experiment / Run objects.
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
mod lifecycle;
mod logging;
mod parameter;
mod run;
mod sealed;

#[cfg(test)]
mod tests;

pub use attachment::{
    detect_file_media_type, AttachmentNotFound, AttachmentTable, Compression,
    DEFAULT_FILE_MEDIA_TYPE,
};
pub use dynamic::{ExperimentDyn, RunDyn, SamplingDyn, SealedRunDyn, SolveDyn};
pub use lifecycle::{ExperimentLifecycle, RunLifecycle};
pub use logging::AttachmentLogger;
pub use parameter::{ParameterValue, RunParameterCell};
pub use run::{FailedSampleRecord, FailedSolveRecord, FinishedSampleRecord, FinishedSolveRecord};
// Local Registry owns the SQLite projection, while Experiment owns validation
// of Experiment manifests/configs before those projection rows are written.
pub(crate) use sealed::experiment_manifest_record_from_artifact;
pub use sealed::{Sampling, SealedRun, Solve};

use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor, TempLocalRegistry};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use anyhow::{ensure, Context, Result};
use oci_spec::image::Descriptor;
use parameter::ParameterSet;
use rmpv::Value as MessagePackValue;
use std::sync::{Mutex, MutexGuard};
use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
    time::{Duration, Instant},
};

// --- Artifact mapping constants ---------------------------------------------

const EXPERIMENT_STATUS_FINISHED: &str = "finished";
const EXPERIMENT_STATUS_DRAFT: &str = "draft";
const EXPERIMENT_STATUS_FAILED: &str = "failed";
const EXPERIMENT_STATUS_INTERRUPTED: &str = "interrupted";

const RUN_PARAMETERS_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-parameters+msgpack";
const EXPERIMENT_ARTIFACT_MEDIA_TYPE: &str = media_types::V1_EXPERIMENT_MEDIA_TYPE;
pub(crate) const EXPERIMENT_CONFIG_MEDIA_TYPE: &str =
    "application/org.ommx.v1.experiment.config+json";

const RUN_STATUS_FINISHED: &str = "finished";
const RUN_STATUS_FAILED: &str = "failed";
const RUN_STATUS_INTERRUPTED: &str = "interrupted";

const SOLVE_STATUS_FINISHED: &str = "finished";
const SOLVE_STATUS_FAILED: &str = "failed";
const SOLVE_STATUS_INTERRUPTED: &str = "interrupted";

const SAMPLING_STATUS_FINISHED: &str = "finished";
const SAMPLING_STATUS_FAILED: &str = "failed";
const SAMPLING_STATUS_INTERRUPTED: &str = "interrupted";

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

    /// Validate status strings reconstructed from serialized Experiment
    /// configs or registry-side Experiment listing projections.
    pub(crate) fn from_config(status: &str) -> Result<Self> {
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

/// Policy controlling rolling draft checkpoints after a [`Run`] closes.
///
/// The policy belongs to the mutable Experiment session. It is not persisted
/// in Experiment artifacts or checkpoints, so new and restored sessions start
/// with [`Self::EveryRunClose`]. Explicit failed or interrupted checkpoints
/// are always published regardless of this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutosavePolicy {
    /// Publish a rolling draft checkpoint after every Run closes.
    #[default]
    EveryRunClose,
    /// Publish after this many additional Runs have closed since the policy
    /// was set or the previous successful autosave.
    ///
    /// A value of zero is invalid and is rejected by
    /// [`Experiment::set_autosave_policy`] and
    /// [`ExperimentDyn::set_autosave_policy`].
    EveryNRuns(u32),
    /// Attempt to publish on the first Run close and then no more than once
    /// per interval. A failed attempt also waits for the interval before retrying.
    MinInterval(Duration),
    /// Do not publish rolling draft checkpoints after Run close.
    Disabled,
}

impl AutosavePolicy {
    fn validate(self) -> Result<()> {
        ensure!(
            !matches!(self, Self::EveryNRuns(0)),
            "AutosavePolicy::EveryNRuns requires a non-zero Run count"
        );
        Ok(())
    }
}

/// Runtime-only scheduling state for Run-close autosaves.
#[derive(Debug, Clone)]
struct AutosaveController {
    policy: AutosavePolicy,
    last_autosaved_run_count: usize,
    last_attempt_at: Option<Instant>,
}

impl AutosaveController {
    fn new(current_run_count: usize) -> Self {
        Self {
            policy: AutosavePolicy::default(),
            last_autosaved_run_count: current_run_count,
            last_attempt_at: None,
        }
    }

    fn set_policy(&mut self, policy: AutosavePolicy, current_run_count: usize) -> Result<()> {
        policy.validate()?;
        self.policy = policy;
        self.last_autosaved_run_count = current_run_count;
        self.last_attempt_at = None;
        Ok(())
    }

    /// Reserve a Run-close autosave attempt before any registry I/O starts.
    ///
    /// `MinInterval` rate-limits attempts, including attempts that later fail.
    /// Other policies continue to advance only after a successful checkpoint.
    fn begin_autosave_attempt(&mut self, now: Instant, current_run_count: usize) -> bool {
        let due = match self.policy {
            AutosavePolicy::EveryRunClose => true,
            AutosavePolicy::EveryNRuns(run_count) => {
                current_run_count.saturating_sub(self.last_autosaved_run_count)
                    >= run_count as usize
            }
            AutosavePolicy::MinInterval(interval) => self
                .last_attempt_at
                .is_none_or(|last| now.saturating_duration_since(last) >= interval),
            AutosavePolicy::Disabled => false,
        };
        if due && matches!(self.policy, AutosavePolicy::MinInterval(_)) {
            self.last_attempt_at = Some(now);
        }
        due
    }

    /// Record a policy-independent checkpoint attempt such as checkpoint
    /// relocation after rename.
    fn record_forced_attempt(&mut self, now: Instant) {
        if matches!(self.policy, AutosavePolicy::MinInterval(_)) {
            self.last_attempt_at = Some(now);
        }
    }

    fn mark_autosaved(&mut self, current_run_count: usize) {
        self.last_autosaved_run_count = current_run_count;
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
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status of one solver call recorded in an Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveStatus {
    /// The adapter returned a Solution.
    Finished,
    /// The adapter raised an error before returning a Solution.
    Failed,
    /// The adapter call was interrupted before returning a Solution.
    Interrupted,
}

impl SolveStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => SOLVE_STATUS_FINISHED,
            Self::Failed => SOLVE_STATUS_FAILED,
            Self::Interrupted => SOLVE_STATUS_INTERRUPTED,
        }
    }

    fn from_config(status: &str) -> Result<Self> {
        match status {
            SOLVE_STATUS_FINISHED => Ok(Self::Finished),
            SOLVE_STATUS_FAILED => Ok(Self::Failed),
            SOLVE_STATUS_INTERRUPTED => Ok(Self::Interrupted),
            _ => {
                crate::bail!(
                    "Solve status is {status}, expected {SOLVE_STATUS_FINISHED}, \
                     {SOLVE_STATUS_FAILED}, or {SOLVE_STATUS_INTERRUPTED}"
                )
            }
        }
    }
}

impl std::fmt::Display for SolveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status of one sampler call recorded in an Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SamplingStatus {
    /// The adapter returned a SampleSet.
    Finished,
    /// The adapter raised an error before returning a SampleSet.
    Failed,
    /// The adapter call was interrupted before returning a SampleSet.
    Interrupted,
}

impl SamplingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => SAMPLING_STATUS_FINISHED,
            Self::Failed => SAMPLING_STATUS_FAILED,
            Self::Interrupted => SAMPLING_STATUS_INTERRUPTED,
        }
    }

    fn from_config(status: &str) -> Result<Self> {
        match status {
            SAMPLING_STATUS_FINISHED => Ok(Self::Finished),
            SAMPLING_STATUS_FAILED => Ok(Self::Failed),
            SAMPLING_STATUS_INTERRUPTED => Ok(Self::Interrupted),
            _ => {
                crate::bail!(
                    "Sampling status is {status}, expected {SAMPLING_STATUS_FINISHED}, \
                     {SAMPLING_STATUS_FAILED}, or {SAMPLING_STATUS_INTERRUPTED}"
                )
            }
        }
    }
}

impl std::fmt::Display for SamplingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A mutable, unsealed experiment session. See the [module documentation](self).
#[derive(Debug)]
pub struct Experiment<'reg> {
    registry: &'reg LocalRegistry,
    state: Mutex<UnsealedExperimentState<'reg>>,
    unresolved_run_behavior: UnresolvedRunBehavior,
}

/// A sealed experiment session whose root artifact manifest has been
/// written and published.
#[derive(Debug, Clone)]
pub struct SealedExperiment<'reg> {
    lifecycle: ExperimentLifecycle,
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
/// parent experiment when a `finish*` method consumes the handle. Ordinary
/// unresolved drop abandons that local state; [`Self::interrupt_on_drop`] and
/// Runs created by [`Experiment::scoped`] opt into best-effort interrupted
/// write-back instead.
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
    samplings: Vec<SamplingEntry<'reg>>,
    next_sampling_id: u64,
    parameters: ParameterSet,
    interrupt_on_drop: bool,
    closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnresolvedRunBehavior {
    Abandon,
    Interrupt,
}

/// Private owner used only to publish an interrupted checkpoint during panic
/// unwind from `Experiment::scoped*`.
struct ScopedExperimentGuard<'reg> {
    experiment: Option<Experiment<'reg>>,
}

impl<'reg> ScopedExperimentGuard<'reg> {
    fn new(experiment: Experiment<'reg>) -> Self {
        Self {
            experiment: Some(experiment),
        }
    }

    fn experiment(&self) -> &Experiment<'reg> {
        self.experiment
            .as_ref()
            .expect("scoped Experiment is present while its callback runs")
    }

    fn take(&mut self) -> Experiment<'reg> {
        self.experiment
            .take()
            .expect("scoped Experiment has exactly one terminal transition")
    }
}

impl Drop for ScopedExperimentGuard<'_> {
    fn drop(&mut self) {
        let Some(experiment) = self.experiment.take() else {
            return;
        };
        if let Err(error) = experiment.commit_checkpoint(ExperimentLifecycle::Interrupted {
            reason: Some("Experiment scope unwound".to_string()),
        }) {
            tracing::warn!(
                error = %error,
                "Failed to publish interrupted Experiment checkpoint during scoped unwind"
            );
        }
    }
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
    lifecycle: RunLifecycle,
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    trace: Option<StoredDescriptor<'reg>>,
    solves: Vec<SolveEntry<'reg>>,
    samplings: Vec<SamplingEntry<'reg>>,
    parameters: ParameterSet,
}

#[derive(Debug, Clone)]
struct SolveEntry<'reg> {
    solve_id: u64,
    status: SolveStatus,
    input: StoredDescriptor<'reg>,
    output: Option<StoredDescriptor<'reg>>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<StoredDescriptor<'reg>>,
}

#[derive(Debug, Clone)]
struct SamplingEntry<'reg> {
    sampling_id: u64,
    status: SamplingStatus,
    input: StoredDescriptor<'reg>,
    output: Option<StoredDescriptor<'reg>>,
    adapter: String,
    adapter_options: String,
    diagnostics: Option<StoredDescriptor<'reg>>,
}

/// Adapter diagnostics payload for one Solve or Sampling.
#[derive(Debug, Clone)]
pub struct AdapterDiagnosticPayload {
    value: MessagePackValue,
}

impl AdapterDiagnosticPayload {
    /// Create a diagnostics payload from MessagePack bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let mut cursor = Cursor::new(&bytes);
        let value = rmpv::decode::read_value(&mut cursor)
            .context("Adapter diagnostic payload must be valid MessagePack")?;
        ensure!(
            cursor.position() == bytes.len() as u64,
            "Adapter diagnostic payload must contain exactly one MessagePack value",
        );
        Self::from_value(value)
    }

    /// Create a diagnostics payload from a decoded MessagePack value.
    pub fn from_value(value: MessagePackValue) -> Result<Self> {
        ensure!(
            matches!(value, MessagePackValue::Array(_)),
            "Adapter diagnostic payload must decode to a MessagePack array",
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
            .context("Failed to encode Adapter diagnostic payload as MessagePack")?;
        Ok(bytes)
    }
}

fn read_adapter_diagnostic_payload(
    record_kind: &str,
    record_id: u64,
    descriptor: &StoredDescriptor<'_>,
) -> Result<(Vec<u8>, AdapterDiagnosticPayload)> {
    descriptor.ensure_media_type(&media_types::diagnostic_msgpack())?;
    let bytes = descriptor.registry().get_blob(descriptor)?;
    let payload = AdapterDiagnosticPayload::new(bytes.clone())
        .with_context(|| format!("Invalid {record_kind} {record_id} diagnostic payload"))?;
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
    /// Manifest annotations written to the root OCI manifest at commit.
    annotations: HashMap<String, String>,
    /// Experiment-space attachments.
    attachments: AttachmentTable<StoredDescriptor<'reg>>,
    runs: BTreeMap<u64, RunEntry<'reg>>,
    next_run_id: u64,
    autosave: AutosaveController,
}

impl<'reg> UnsealedExperimentState<'reg> {
    fn autosave_after_run_close(
        &mut self,
        registry: &'reg LocalRegistry,
    ) -> Result<Option<LocalArtifact<'reg>>> {
        let run_count = self.runs.len();
        if !self
            .autosave
            .begin_autosave_attempt(Instant::now(), run_count)
        {
            return Ok(None);
        }
        let artifact = self.autosave_checkpoint(registry)?;
        self.autosave.mark_autosaved(run_count);
        Ok(Some(artifact))
    }
}

impl Experiment<'static> {
    /// Start a new experiment session backed by the user's default
    /// Local Registry. The committed artifact is published under the
    /// resolved `name`.
    pub fn new(name: impl Into<Name>) -> Result<Self> {
        let registry = LocalRegistry::shared_default()?;
        Self::with_registry(registry, name)
    }

    /// Run a lifecycle-safe Experiment callback in the default Local Registry.
    ///
    /// A successful callback commits the Experiment. A returned error publishes
    /// a best-effort failed checkpoint and returns the original callback error.
    /// Panic unwind publishes a best-effort interrupted checkpoint. Runs opened
    /// directly or through [`Experiment::scoped_run`] are finalized as
    /// interrupted if they remain unresolved during callback unwind.
    /// This method only writes to the Local Registry; it never pushes remotely.
    /// As with any RAII API, deliberately leaking a Run with
    /// [`std::mem::forget`] bypasses its drop fallback.
    ///
    /// A Run cannot escape the callback because it borrows the scoped
    /// Experiment:
    ///
    /// ```compile_fail
    /// use ommx::experiment::{Experiment, Name};
    ///
    /// let mut escaped_run = None;
    /// let _ = Experiment::scoped(Name::Anonymous, |experiment| {
    ///     escaped_run = Some(experiment.run()?);
    ///     Ok(())
    /// });
    /// ```
    pub fn scoped(
        name: impl Into<Name>,
        f: impl FnOnce(&Experiment<'static>) -> Result<()>,
    ) -> Result<SealedExperiment<'static>> {
        let registry = LocalRegistry::shared_default()?;
        Self::scoped_with_registry(registry, name, f)
    }
}

impl<'reg> Experiment<'reg> {
    /// Run a lifecycle-safe Experiment callback against an explicit Local
    /// Registry.
    ///
    /// Calling this method opts the created Experiment and its Runs into
    /// best-effort interrupted finalization during unresolved drop. Ordinary
    /// [`Experiment::with_registry`] and [`Experiment::run`] usage retains the
    /// existing abandon-on-drop behavior unless a Run explicitly calls
    /// [`Run::interrupt_on_drop`].
    pub fn scoped_with_registry(
        registry: &'reg LocalRegistry,
        name: impl Into<Name>,
        f: impl FnOnce(&Experiment<'reg>) -> Result<()>,
    ) -> Result<SealedExperiment<'reg>> {
        let experiment =
            Self::with_registry_and_run_behavior(registry, name, UnresolvedRunBehavior::Interrupt)?;
        let mut guard = ScopedExperimentGuard::new(experiment);
        match f(guard.experiment()) {
            Ok(()) => guard.take().commit(),
            Err(error) => {
                if let Err(checkpoint_error) =
                    guard.take().commit_checkpoint(ExperimentLifecycle::Failed {
                        reason: Some(error.to_string()),
                    })
                {
                    tracing::warn!(
                        error = %checkpoint_error,
                        "Failed to publish failed Experiment checkpoint after callback error"
                    );
                }
                Err(error)
            }
        }
    }

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
        Self::with_registry_and_run_behavior(registry, name, UnresolvedRunBehavior::Abandon)
    }

    fn with_registry_and_run_behavior(
        registry: &'reg LocalRegistry,
        name: impl Into<Name>,
        unresolved_run_behavior: UnresolvedRunBehavior,
    ) -> Result<Self> {
        let image_name = name.into().resolve(registry)?;
        Ok(Experiment {
            registry,
            state: Mutex::new(UnsealedExperimentState {
                image_name,
                subject: None,
                annotations: HashMap::new(),
                attachments: AttachmentTable::new(),
                runs: BTreeMap::new(),
                next_run_id: 0,
                autosave: AutosaveController::new(0),
            }),
            unresolved_run_behavior,
        })
    }

    /// Concrete Local Registry image name this Experiment will publish
    /// to when committed.
    pub fn image_name(&self) -> ImageRef {
        self.lock_state().image_name.clone()
    }

    /// Set a manifest annotation on the Experiment artifact committed by this session.
    pub fn set_annotation(&self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let key = key.into();
        ensure!(
            !crate::is_reserved_annotation_key(&key),
            "Annotation key `{key}` is reserved for OMMX metadata"
        );
        self.lock_state().annotations.insert(key, value.into());
        Ok(())
    }

    /// Set the policy for rolling draft checkpoints after a Run closes.
    ///
    /// Changing the policy resets its schedule at the current closed-Run
    /// count. [`AutosavePolicy::EveryNRuns`] therefore counts Runs closed
    /// after this call. [`AutosavePolicy::MinInterval`] checkpoints the first
    /// subsequently closed Run immediately. A zero `EveryNRuns` count is
    /// rejected without changing the current policy.
    pub fn set_autosave_policy(&self, policy: AutosavePolicy) -> Result<()> {
        let mut state = self.lock_state();
        let run_count = state.runs.len();
        state.autosave.set_policy(policy, run_count)
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
            samplings: Vec::new(),
            next_sampling_id: 0,
            parameters: ParameterSet::new(),
            interrupt_on_drop: self.unresolved_run_behavior == UnresolvedRunBehavior::Interrupt,
            closed: false,
        })
    }

    /// Run one lifecycle-safe callback against a new Run.
    ///
    /// A successful callback finishes the Run. A returned error finishes it as
    /// failed and returns the original callback error. Panic unwind or another
    /// unresolved drop finishes it as interrupted. Partial parameters and
    /// attachments are preserved on failed and interrupted paths.
    pub fn scoped_run<T>(&self, f: impl FnOnce(&mut Run<'_, 'reg>) -> Result<T>) -> Result<T> {
        let mut run = self.run()?.interrupt_on_drop();
        match f(&mut run) {
            Ok(value) => {
                run.finish()?;
                Ok(value)
            }
            Err(error) => {
                if let Err(finish_error) = run.finish_failed_with_reason(error.to_string()) {
                    tracing::warn!(
                        error = %finish_error,
                        "Failed to finish failed Run after callback error"
                    );
                }
                Err(error)
            }
        }
    }

    fn push_closed_run(&self, run: RunEntry<'reg>) -> Result<()> {
        let mut state = self.lock_state();
        if state.runs.contains_key(&run.run_id) {
            crate::bail!("Run {} has already been registered", run.run_id);
        }
        state.runs.insert(run.run_id, run);
        if let Err(error) = state.autosave_after_run_close(self.registry) {
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
        let (registry, state) = self.into_parts();
        let artifact = state.commit(registry)?;
        SealedExperiment::from_artifact(artifact)
    }

    fn commit_checkpoint(self, lifecycle: ExperimentLifecycle) -> Result<LocalArtifact<'reg>> {
        let (registry, state) = self.into_parts();
        state.commit_checkpoint(registry, lifecycle)
    }

    fn into_parts(self) -> (&'reg LocalRegistry, UnsealedExperimentState<'reg>) {
        let Experiment {
            registry,
            state,
            unresolved_run_behavior: _,
        } = self;
        let state = match state.into_inner() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!("Experiment state mutex was poisoned; consuming inner state");
                poisoned.into_inner()
            }
        };
        (registry, state)
    }
}

impl<'reg> logging::AttachmentLoggerStorage for &Experiment<'reg> {
    type Descriptor = StoredDescriptor<'reg>;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R> {
        f(self.registry)
    }

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R> {
        let mut state = self.lock_state();
        f(&mut state.attachments)
    }

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor> {
        self.registry.stored_descriptor(descriptor)
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
    /// attachments, runs, solves, samplings, and run parameters are carried into
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
                    status: solve.status().clone(),
                    input: solve.input_descriptor().clone(),
                    output: solve.output_descriptor().cloned(),
                    adapter: solve.adapter().to_string(),
                    adapter_options: solve.adapter_options().to_string(),
                    diagnostics: solve.diagnostic_descriptor().cloned(),
                })
                .collect();
            let samplings = run
                .samplings()
                .iter()
                .map(|sampling| SamplingEntry {
                    sampling_id: sampling.sampling_id(),
                    status: sampling.status().clone(),
                    input: sampling.input_descriptor().clone(),
                    output: sampling.output_descriptor().cloned(),
                    adapter: sampling.adapter().to_string(),
                    adapter_options: sampling.adapter_options().to_string(),
                    diagnostics: sampling.diagnostic_descriptor().cloned(),
                })
                .collect();
            runs.insert(
                run.run_id(),
                RunEntry {
                    run_id: run.run_id(),
                    lifecycle: run.lifecycle().clone(),
                    attachments: run.attachment_table().clone(),
                    trace: run.trace_descriptor().cloned(),
                    solves,
                    samplings,
                    parameters,
                },
            );
        }

        Ok(Experiment {
            registry,
            state: Mutex::new(UnsealedExperimentState {
                image_name,
                subject,
                annotations: HashMap::new(),
                attachments: self.attachments.clone(),
                next_run_id: next_run_id(runs.keys().copied())?,
                autosave: AutosaveController::new(runs.len()),
                runs,
            }),
            unresolved_run_behavior: UnresolvedRunBehavior::Abandon,
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
