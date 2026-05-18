//! Experiment / Run session model.
//!
//! An [`Experiment`] is a mutable session that groups a set of named
//! payloads (records) — instances, solutions, sample sets, JSON values,
//! or caller-defined media types — together with one or more [`Run`]s.
//! Records belong either
//! to the *experiment space* (shared by the whole experiment) or to a
//! *run space* (owned by a single [`Run`]).
//! Run parameters are separate table data: [`Run::log_parameter`] records
//! bool / int64 / float64 / string scalar values for comparison views,
//! and commit materialises them as a typed column-oriented aggregate
//! run-parameter layer instead of individual Records.
//!
//! Each `log_*` call writes its payload to the Local Registry's
//! content-addressed BlobStore immediately, keeping only
//! [`crate::artifact::local_registry::StoredDescriptor`] values in
//! memory. Until commit, the experiment is unsealed: some or all
//! component blobs may already be stored, but no root manifest has been
//! stored for the whole experiment. [`Experiment::commit`] seals that
//! mutable session into a single immutable OMMX Artifact whose manifest
//! references those already-stored blobs. The registry-level operation
//! that updates the image ref is publish; the Experiment-level
//! operation remains commit.
//!
//! ```ignore
//! use ommx::experiment::Experiment;
//!
//! let exp = Experiment::new("ghcr.io/jij-inc/ommx/scip_reblock115:latest")?;
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
//! The module is split by data terms: `run` contains `Run` and
//! `RunEntry`, `record` contains `RecordRef`, `parameter` contains
//! run-parameter table data, `index` contains the experiment index
//! layer data, and `artifact` maps the unsealed experiment state onto
//! an OMMX Artifact.

mod artifact;
mod parameter;
mod record;
mod run;

#[cfg(test)]
mod tests;

pub use parameter::ParameterValue;

use crate::artifact::local_registry::{LocalRegistry, TempLocalRegistry};
use crate::artifact::{media_types, ImageRef, LocalArtifact};
use crate::{Instance, SampleSet, Solution};
use anyhow::Result;
use oci_spec::image::MediaType;
use record::{
    encode_json, json_media_type, store_record_ref, upsert_record_ref, RecordRef, RecordSpace,
};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

// --- Artifact mapping constants ---------------------------------------------

const ARTIFACT_KIND_EXPERIMENT: &str = "experiment";
const EXPERIMENT_SCHEMA_V1: &str = "v1";
const EXPERIMENT_STATUS_FINISHED: &str = "finished";

const ANN_ARTIFACT_KIND: &str = "org.ommx.artifact.kind";
const ANN_EXPERIMENT_SCHEMA: &str = "org.ommx.experiment.schema";
const ANN_EXPERIMENT_STATUS: &str = "org.ommx.experiment.status";
const ANN_SPACE: &str = "org.ommx.experiment.space";
const ANN_RUN_ID: &str = "org.ommx.experiment.run_id";
const ANN_LAYER: &str = "org.ommx.experiment.layer";
const ANN_RECORD_NAME: &str = "org.ommx.record.name";

const RUN_PARAMETERS_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-parameters+json";
const LAYER_KIND_RUN_PARAMETERS: &str = "run-parameters";

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
    artifact: LocalArtifact<'reg>,
}

/// A handle to a single run within an [`Experiment`].
///
/// A `Run` borrows its parent experiment immutably for `'exp`. It
/// writes payload bytes to the registry CAS immediately, keeps
/// run-scoped records / parameters locally, and writes back to the
/// parent experiment only when [`Self::finish`] consumes the handle.
/// This lets multiple runs be open at once while Rust prevents
/// committing the parent experiment before live run handles are closed
/// or dropped.
#[derive(Debug)]
pub struct Run<'exp, 'reg> {
    experiment: &'exp Experiment<'reg>,
    run_id: u64,
    records: Vec<RecordRef<'reg>>,
    parameters: BTreeMap<String, ParameterValue>,
}

/// A closed logical Run recorded in an unsealed Experiment.
///
/// `Run<'exp>` is the live handle: it borrows the parent Experiment and
/// accepts run-scoped records and parameters. `RunEntry` is the row
/// stored by the Experiment after `Run::finish` consumes that handle.
/// Commit later projects it to aggregate parameter and record index
/// layers.
#[derive(Debug)]
struct RunEntry<'reg> {
    run_id: u64,
    records: Vec<RecordRef<'reg>>,
    parameters: BTreeMap<String, ParameterValue>,
}

/// Mutable experiment state before the root manifest is sealed. A live
/// [`Run`] borrows the parent experiment while it adds run-scoped
/// records. Closed runs are stored as [`RunEntry`] values.
#[derive(Debug)]
struct UnsealedExperimentState<'reg> {
    /// Image name the committed Experiment artifact is published
    /// under. Experiment identity is the Local Registry ref; there is
    /// no separate experiment-name field in the artifact model.
    image_name: ImageRef,
    /// Experiment-space records.
    records: Vec<RecordRef<'reg>>,
    runs: BTreeMap<u64, RunEntry<'reg>>,
    next_run_id: u64,
}

impl Experiment<'static> {
    /// Start a new experiment session backed by the user's default
    /// Local Registry. The committed artifact is published under
    /// `image_name`.
    pub fn new(image_name: impl AsRef<str>) -> Result<Self> {
        let registry = LocalRegistry::shared_default()?;
        let image_name = ImageRef::parse(image_name.as_ref())?;
        Ok(Self::with_registry(image_name, registry))
    }

    /// Start a new experiment session backed by the user's default
    /// Local Registry and publish it under an anonymous image name.
    pub fn anonymous() -> Result<Self> {
        let registry = LocalRegistry::shared_default()?;
        Self::with_anonymous_registry(registry)
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
        image_name: impl AsRef<str>,
        f: impl FnOnce(Experiment<'_>) -> anyhow::Result<T>,
    ) -> Result<T> {
        let temp = TempLocalRegistry::new()?;
        let image_name = ImageRef::parse(image_name.as_ref())?;
        let experiment = Experiment::with_registry(image_name, temp.registry());
        f(experiment)
    }

    /// Start a new experiment session against an explicit Local
    /// Registry and publish it under an anonymous image name generated
    /// by that registry.
    pub fn with_anonymous_registry(registry: &'reg LocalRegistry) -> Result<Self> {
        let image_name = registry.synthesize_anonymous_experiment_image_name()?;
        Ok(Self::with_registry(image_name, registry))
    }

    /// Start a new experiment session against an explicit Local
    /// Registry. The committed artifact is published under
    /// `image_name`.
    pub fn with_registry(image_name: ImageRef, registry: &'reg LocalRegistry) -> Self {
        Experiment {
            registry,
            state: Mutex::new(UnsealedExperimentState {
                image_name,
                records: Vec::new(),
                runs: BTreeMap::new(),
                next_run_id: 0,
            }),
        }
    }

    /// Start a new [`Run`]. Each run gets a fresh 0-based `run_id`.
    pub fn run(&self) -> Result<Run<'_, 'reg>> {
        let mut state = self.lock_state();
        let run_id = state.next_run_id;
        state.next_run_id += 1;
        Ok(Run {
            experiment: self,
            run_id,
            records: Vec::new(),
            parameters: BTreeMap::new(),
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
            self.registry,
            RecordSpace::Experiment,
            None,
            name,
            media_type,
            bytes,
        )?;
        let mut state = self.lock_state();
        upsert_record_ref(&mut state.records, record_ref);
        Ok(())
    }

    fn push_closed_run(&self, run: RunEntry<'reg>) -> Result<()> {
        let mut state = self.lock_state();
        if state.runs.contains_key(&run.run_id) {
            crate::bail!("Run {} has already been recorded", run.run_id);
        }
        state.runs.insert(run.run_id, run);
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
        Ok(SealedExperiment { artifact })
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
}
