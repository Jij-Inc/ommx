//! Experiment / Run session model.
//!
//! An [`Experiment`] is a mutable session that groups a set of named
//! payloads (records) — instances, solutions, sample sets, JSON values,
//! or caller-defined media types — together with one or more [`Run`]s.
//! Records belong either
//! to the *experiment space* (shared by the whole experiment) or to a
//! *run space* (owned by a single [`Run`]).
//! Run parameters are separate table data: [`Run::log_parameter`] records
//! scalar values for comparison views, and commit materialises them as
//! an aggregate run-parameter layer instead of individual Records.
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
//! The module is split into three concerns: `model` holds the
//! in-memory state types, `session` the public `Experiment` / `Run`
//! handles and their `log_*` API, and `commit` the mapping onto an
//! OMMX Artifact.

mod commit;
mod model;
mod session;

#[cfg(test)]
mod tests;

pub use session::{Experiment, Run, SealedExperiment};

use anyhow::Result;
use oci_spec::image::{Descriptor, DescriptorBuilder, Digest, MediaType};
use std::collections::HashMap;

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

/// Build an OCI layer descriptor from a CAS-written blob plus the
/// experiment / record annotations. Shared by record staging (in
/// `session`) and the commit-time aggregate layers (in `commit`).
fn build_descriptor(
    media_type: MediaType,
    digest: &Digest,
    size: u64,
    annotations: HashMap<String, String>,
) -> Result<Descriptor> {
    DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest.clone())
        .size(size)
        .annotations(annotations)
        .build()
        .map_err(|e| crate::error!("Failed to build OCI descriptor: {e}"))
}
