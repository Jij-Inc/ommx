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
//! let exp = Experiment::new("scip_reblock115")?;
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
//! The module is split by domain terms: `run` contains the `Run`
//! lifecycle and run-scoped `log_*` API, `record` contains Record
//! references, `parameter` contains run parameter scalar values and
//! table aggregation, and `commit` maps the unsealed experiment state
//! onto an OMMX Artifact.

mod commit;
mod parameter;
mod record;
mod run;

#[cfg(test)]
mod tests;

pub use parameter::ParameterValue;
pub use run::Run;

use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::ImageRef;
use crate::artifact::LocalArtifact;
use record::RecordRef;
use run::RunEntry;
use std::sync::Mutex;

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
const ANN_RECORD_NAME: &str = "org.ommx.record.name";

const EXPERIMENT_INDEX_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment+json";
const RUN_PARAMETERS_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-parameters+json";
const RUN_ATTRIBUTES_MEDIA_TYPE: &str = "application/org.ommx.v1.experiment.run-attributes+json";
const LAYER_KIND_INDEX: &str = "index";
const LAYER_KIND_RUN_PARAMETERS: &str = "run-parameters";
const LAYER_KIND_RUN_ATTRIBUTES: &str = "run-attributes";

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

/// Mutable experiment state before the root manifest is sealed. A live
/// [`Run`] borrows the parent experiment while it adds run-scoped
/// records. Closed runs are stored as [`RunEntry`] values.
#[derive(Debug)]
struct UnsealedExperimentState<'reg> {
    name: String,
    /// Image name the committed artifact is published under. `None`
    /// means an anonymous name is synthesised at commit time.
    requested_ref: Option<ImageRef>,
    /// Experiment-space records.
    records: Vec<RecordRef<'reg>>,
    runs: Vec<RunEntry<'reg>>,
    next_run_id: u64,
}
