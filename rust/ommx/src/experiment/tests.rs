//! Tests for the experiment session model.

use super::model::RunStatus;
use super::{
    Experiment, ANN_ARTIFACT_KIND, ANN_EXPERIMENT_NAME, ANN_EXPERIMENT_SCHEMA,
    ANN_EXPERIMENT_STATUS, ANN_LAYER, ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE,
    ARTIFACT_KIND_EXPERIMENT, EXPERIMENT_SCHEMA_V1, EXPERIMENT_STATUS_FINISHED, LAYER_KIND_INDEX,
    LAYER_KIND_RUN_ATTRIBUTES,
};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::media_types;
use crate::Instance;
use oci_spec::image::{Descriptor, MediaType};
use serde_json::json;
use std::sync::Arc;
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

/// `run()` hands out fresh 0-based ids; `finish()` / `fail()` record the
/// final status and elapsed time, and re-closing is a no-op.
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

/// `log_*` writes the payload to the BlobStore immediately, before any
/// commit advances a public ref.
#[test]
fn log_writes_blob_to_blobstore_immediately() {
    let (_dir, experiment) = temp_experiment("eager-write");
    let run = experiment.run().unwrap();
    run.log_json("solver", json!("scip")).unwrap();

    let digest = {
        let state = experiment.state.lock().unwrap();
        assert_eq!(state.runs[0].records.len(), 1);
        let digest = state.runs[0].records[0].descriptor.digest().clone();
        assert!(state.staged_blobs.contains_key(&digest));
        digest
    };
    assert!(experiment.registry.blobs().exists(&digest).unwrap());
    assert!(!experiment.is_committed().unwrap());
}

/// Logging the same `(space, media type, name)` again replaces the
/// record.
#[test]
fn log_upserts_same_space_media_type_name() {
    let (_dir, experiment) = temp_experiment("upsert");
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    experiment.log_json("dataset", json!("qplib")).unwrap();

    let instance: Instance =
        crate::random::random_deterministic(crate::InstanceParameters::default_lp());
    experiment.log_instance("dataset", &instance).unwrap();

    let state = experiment.state.lock().unwrap();
    assert_eq!(state.records.len(), 2);
    let json_record = state
        .records
        .iter()
        .find(|record| {
            record.descriptor.media_type() == &MediaType::Other("application/json".into())
        })
        .unwrap();
    let bytes = experiment
        .registry
        .blobs()
        .read_bytes(json_record.descriptor.digest())
        .unwrap();
    assert_eq!(bytes, serde_json::to_vec(&json!("qplib")).unwrap());
}

/// `commit()` seals the session into an OMMX Artifact whose manifest and
/// layer annotations describe the experiment / run records.
#[test]
fn commit_produces_experiment_artifact() {
    let (_dir, experiment) = temp_experiment("commit");
    experiment.log_json("dataset", json!("miplib2017")).unwrap();

    let instance: Instance =
        crate::random::random_deterministic(crate::InstanceParameters::default_lp());
    let run = experiment.run().unwrap();
    run.log_instance("candidate", &instance).unwrap();
    run.log_json("config", json!({ "relaxed": true })).unwrap();
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
        dataset.media_type(),
        &MediaType::Other("application/json".into())
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
    assert_eq!(candidate.media_type(), &media_types::v1_instance());
    assert_eq!(
        artifact.get_blob(candidate.digest()).unwrap(),
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
    run.log_json("seed", json!(0)).unwrap();
    run.finish().unwrap();
    experiment.commit().unwrap();

    assert!(experiment.log_json("late", json!(1)).is_err());
    assert!(experiment.run().is_err());
    assert!(run.log_json("late", json!(1)).is_err());
}

/// `commit()` is idempotent: the second call returns the artifact
/// produced by the first.
#[test]
fn commit_is_idempotent() {
    let (_dir, experiment) = temp_experiment("idempotent");
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
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
    run0.log_json("candidate", payload.clone()).unwrap();
    run0.finish().unwrap();

    let run1 = experiment.run().unwrap();
    run1.log_json("candidate", payload.clone()).unwrap();
    run1.finish().unwrap();

    let artifact = experiment.commit().unwrap();
    let layers = artifact.layers().unwrap();

    let candidates: Vec<&Descriptor> = layers
        .iter()
        .filter(|layer| layer_annotation(layer, ANN_RECORD_NAME).as_deref() == Some("candidate"))
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

/// Caller-defined payload types are represented directly by OCI media
/// type, without an additional OMMX record-kind axis.
#[test]
fn log_record_accepts_caller_defined_media_type() {
    let (_dir, experiment) = temp_experiment("custom-media-type");
    let media_type = MediaType::Other("application/vnd.jijmodeling.model+json".to_string());
    experiment
        .log_record("source-model", media_type.clone(), br#"{"variables": []}"#)
        .unwrap();

    let artifact = experiment.commit().unwrap();
    let layers = artifact.layers().unwrap();
    let source_model = find_layer(&layers, ANN_RECORD_NAME, "source-model");
    assert_eq!(source_model.media_type(), &media_type);
    assert_eq!(
        artifact.get_blob(source_model.digest()).unwrap(),
        br#"{"variables": []}"#
    );
}
