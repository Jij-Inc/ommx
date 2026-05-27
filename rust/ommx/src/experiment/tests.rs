//! Tests for the experiment session model.

use super::config::{ExperimentConfig, ExperimentConfigRun, LayerRef};
use super::UnsealedExperimentState;
use super::{
    Experiment, ExperimentDyn, Name, ParameterValue, SealedExperiment, ANN_ATTACHMENT_NAME,
    ANN_LAYER, ANN_RUN_ID, ANN_SPACE, EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_STATUS_FINISHED,
    LAYER_KIND_RUN_PARAMETERS, RUN_PARAMETERS_MEDIA_TYPE,
};
use crate::artifact::local_registry::{StoredDescriptor, UnsealedArtifact};
use crate::artifact::{media_types, AsArtifact, ImageRef, LocalArtifact, LocalRegistryHandle};
use crate::{Evaluate, Function, Instance, Sense};
use oci_spec::image::{Descriptor, MediaType};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

fn with_temp_experiment<T>(f: impl FnOnce(Experiment<'_>) -> anyhow::Result<T>) -> T {
    Experiment::with_temp_local_registry(Name::Anonymous, f).unwrap()
}

fn with_unsealed_state<T>(
    experiment: &Experiment<'_>,
    f: impl FnOnce(&UnsealedExperimentState<'_>) -> T,
) -> T {
    let state = experiment.state.lock().expect("experiment state lock");
    f(&state)
}

fn layer_annotation(layer: &Descriptor, key: &str) -> Option<String> {
    layer
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(key).cloned())
}

/// Find the single layer whose `annotations[key]` equals `value`.
fn find_layer<'a, 'reg>(
    layers: &'a [StoredDescriptor<'reg>],
    key: &str,
    value: &str,
) -> &'a StoredDescriptor<'reg> {
    let matches: Vec<&StoredDescriptor<'reg>> = layers
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

fn blob_bytes(artifact: &LocalArtifact<'_>, descriptor: &StoredDescriptor<'_>) -> Vec<u8> {
    artifact.get_blob(descriptor).unwrap()
}

/// `run()` hands out fresh 0-based ids; `finish()` consumes the run
/// handle and registers the closed run.
#[test]
fn run_lifecycle_assigns_ids_and_registers_closed_runs() {
    with_temp_experiment(|experiment| {
        {
            let run0 = experiment.run().unwrap();
            assert_eq!(run0.run_id(), 0);
            run0.finish().unwrap();
        }
        {
            let run1 = experiment.run().unwrap();
            assert_eq!(run1.run_id(), 1);
            run1.finish().unwrap();
        }

        with_unsealed_state(&experiment, |state| {
            assert_eq!(state.runs.get(&0).unwrap().run_id, 0);
            assert_eq!(state.runs.get(&1).unwrap().run_id, 1);
        });
        Ok(())
    });
}

#[test]
fn run_rejects_exhausted_run_id_space() {
    with_temp_experiment(|experiment| {
        {
            let mut state = experiment.state.lock().expect("experiment state lock");
            state.next_run_id = u64::MAX;
        }

        let err = experiment
            .run()
            .expect_err("u64::MAX cannot be allocated as a run_id");
        assert!(err.to_string().contains("Run ID space is exhausted"));
        Ok(())
    });
}

/// Runs borrow the parent experiment immutably, so several runs can be
/// built before any of them writes back at close.
#[test]
fn runs_can_be_open_concurrently_and_write_back_on_close() {
    with_temp_experiment(|experiment| {
        let mut run0 = experiment.run().unwrap();
        let mut run1 = experiment.run().unwrap();

        run0.log_json("candidate", json!("formulation-a")).unwrap();
        run1.log_json("candidate", json!("formulation-b")).unwrap();

        assert_eq!(
            with_unsealed_state(&experiment, |state| state.runs.len()),
            0
        );
        run1.finish().unwrap();
        assert_eq!(
            with_unsealed_state(&experiment, |state| state.runs.len()),
            1
        );
        run0.finish().unwrap();

        with_unsealed_state(&experiment, |state| {
            assert_eq!(state.runs.len(), 2);
            let run_ids = state.runs.keys().copied().collect::<Vec<_>>();
            assert_eq!(run_ids, vec![0, 1]);
        });

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        assert_eq!(
            layers
                .iter()
                .filter(|layer| {
                    layer_annotation(layer, ANN_ATTACHMENT_NAME).as_deref() == Some("candidate")
                })
                .map(|layer| layer_annotation(layer, ANN_RUN_ID).unwrap())
                .collect::<Vec<_>>(),
            vec!["0".to_string(), "1".to_string()]
        );
        Ok(())
    });
}

/// `log_*` writes the payload to the BlobStore immediately, before any
/// commit advances a public ref.
#[test]
fn log_writes_blob_to_blobstore_immediately() {
    with_temp_experiment(|experiment| {
        {
            let mut run = experiment.run().unwrap();
            run.log_json("solver", json!("scip")).unwrap();
            run.finish().unwrap();
        }

        let digest = with_unsealed_state(&experiment, |state| {
            let run = state.runs.get(&0).unwrap();
            assert_eq!(run.attachments.len(), 1);
            run.attachments[0].digest().clone()
        });
        assert!(experiment.registry.blobs().exists(&digest).unwrap());
        Ok(())
    });
}

/// Attachment descriptors are append-only within the Experiment/Run state.
/// BlobStore still deduplicates byte-identical payloads by digest, but
/// the Experiment model preserves each log call as a descriptor entry.
#[test]
fn log_preserves_attachment_descriptor_log_order() {
    with_temp_experiment(|experiment| {
        experiment.log_json("dataset", json!("miplib2017")).unwrap();
        experiment.log_json("dataset", json!("qplib")).unwrap();

        let instance: Instance =
            crate::random::random_deterministic(crate::InstanceParameters::default_lp());
        experiment.log_instance("dataset", &instance).unwrap();

        let json_digests = with_unsealed_state(&experiment, |state| {
            assert_eq!(state.attachments.len(), 3);
            assert_eq!(
                state
                    .attachments
                    .iter()
                    .map(|attachment| layer_annotation(attachment, ANN_ATTACHMENT_NAME).unwrap())
                    .collect::<Vec<_>>(),
                vec![
                    "dataset".to_string(),
                    "dataset".to_string(),
                    "dataset".to_string()
                ]
            );
            assert_eq!(
                state
                    .attachments
                    .iter()
                    .map(|attachment| attachment.media_type().clone())
                    .collect::<Vec<_>>(),
                vec![
                    MediaType::Other("application/json".into()),
                    MediaType::Other("application/json".into()),
                    media_types::v1_instance(),
                ]
            );
            state
                .attachments
                .iter()
                .take(2)
                .map(|attachment| attachment.digest().clone())
                .collect::<Vec<_>>()
        });
        let first_bytes = experiment
            .registry
            .blobs()
            .read_bytes(&json_digests[0])
            .unwrap();
        let second_bytes = experiment
            .registry
            .blobs()
            .read_bytes(&json_digests[1])
            .unwrap();
        assert_eq!(
            first_bytes,
            serde_json::to_vec(&json!("miplib2017")).unwrap()
        );
        assert_eq!(second_bytes, serde_json::to_vec(&json!("qplib")).unwrap());
        Ok(())
    });
}

/// `commit()` seals the session into an OMMX Artifact whose manifest and
/// layer annotations describe the experiment / run attachments.
#[test]
fn commit_produces_experiment_artifact() {
    with_temp_experiment(|experiment| {
        experiment.log_json("dataset", json!("miplib2017")).unwrap();

        let instance: Instance =
            crate::random::random_deterministic(crate::InstanceParameters::default_lp());
        {
            let mut run = experiment.run().unwrap();
            run.log_instance("candidate", &instance).unwrap();
            run.log_json("config", json!({ "relaxed": true })).unwrap();
            run.finish().unwrap();
        }

        let sealed = experiment.commit().unwrap();
        let artifact = sealed.artifact();

        let annotations = artifact.annotations().unwrap();
        assert!(annotations.is_empty());

        let config = artifact.stored_config().unwrap();
        assert_eq!(
            config.media_type(),
            &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string())
        );
        let config_json: serde_json::Value =
            serde_json::from_slice(&blob_bytes(&artifact, &config)).unwrap();
        assert_eq!(
            config_json.get("status").and_then(|value| value.as_str()),
            Some(EXPERIMENT_STATUS_FINISHED)
        );

        // 3 attachments (1 experiment-space + 2 run-space) + run-parameters.
        let layers = artifact.layers().unwrap();
        assert_eq!(layers.len(), 4);

        let dataset = find_layer(&layers, ANN_ATTACHMENT_NAME, "dataset");
        assert_eq!(
            layer_annotation(dataset, ANN_SPACE).as_deref(),
            Some("experiment")
        );
        assert_eq!(
            dataset.media_type(),
            &MediaType::Other("application/json".into())
        );
        assert!(layer_annotation(dataset, ANN_RUN_ID).is_none());

        let candidate = find_layer(&layers, ANN_ATTACHMENT_NAME, "candidate");
        assert_eq!(
            layer_annotation(candidate, ANN_SPACE).as_deref(),
            Some("run")
        );
        assert_eq!(
            layer_annotation(candidate, ANN_RUN_ID).as_deref(),
            Some("0")
        );
        assert_eq!(candidate.media_type(), &media_types::v1_instance());
        assert_eq!(blob_bytes(&artifact, candidate), instance.to_bytes());

        // Aggregate layers are not tagged as attachments.
        let run_params = find_layer(&layers, ANN_LAYER, LAYER_KIND_RUN_PARAMETERS);
        assert!(layer_annotation(run_params, ANN_SPACE).is_none());

        // Config stores the Experiment structure; layers are payloads referenced from it.
        assert_eq!(
            artifact.get_manifest().unwrap().config().media_type(),
            &MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string())
        );
        Ok(())
    });
}

/// Run parameters are stored as table data, not as Attachments. Re-logging
/// the same name updates the cell for that run.
#[test]
fn log_parameter_materializes_run_parameter_table() {
    with_temp_experiment(|experiment| {
        {
            let mut run0 = experiment.run().unwrap();
            run0.log_parameter("solver", "scip").unwrap();
            run0.log_parameter("time_limit", 10.0).unwrap();
            run0.log_parameter("time_limit", 20.0).unwrap();
            run0.finish().unwrap();
        }
        {
            let mut run1 = experiment.run().unwrap();
            run1.log_parameter("solver", "highs").unwrap();
            run1.log_parameter("presolve", true).unwrap();
            run1.finish().unwrap();
        }

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        let run_params = find_layer(&layers, ANN_LAYER, LAYER_KIND_RUN_PARAMETERS);
        assert!(layer_annotation(run_params, ANN_ATTACHMENT_NAME).is_none());
        let bytes = blob_bytes(&artifact, run_params);
        let table: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(
            table,
            json!({
                "columns": {
                    "presolve": {
                        "type": "bool",
                        "values": {
                            "1": true,
                        },
                    },
                    "solver": {
                        "type": "string",
                        "values": {
                            "0": "scip",
                            "1": "highs",
                        },
                    },
                    "time_limit": {
                        "type": "float64",
                        "values": {
                            "0": 20.0,
                        },
                    },
                },
            })
        );
        Ok(())
    });
}

#[test]
fn loaded_experiment_reads_attachments_and_run_parameters() {
    with_temp_experiment(|experiment| {
        experiment.log_json("dataset", json!("miplib2017")).unwrap();

        {
            let mut run0 = experiment.run().unwrap();
            run0.log_parameter("solver", "scip").unwrap();
            run0.log_parameter("time_limit", 20.0).unwrap();
            run0.log_json("candidate", json!("formulation-a")).unwrap();
            run0.finish().unwrap();
        }
        {
            let mut run1 = experiment.run().unwrap();
            run1.log_parameter("solver", "highs").unwrap();
            run1.log_parameter("presolve", true).unwrap();
            run1.finish().unwrap();
        }

        let artifact = experiment.commit().unwrap().into_artifact();
        let loaded = SealedExperiment::from_artifact(artifact.clone()).unwrap();

        assert!(loaded
            .experiment_attachments()
            .iter()
            .any(
                |attachment| layer_annotation(attachment, ANN_ATTACHMENT_NAME).as_deref()
                    == Some("dataset")
                    && attachment.media_type() == &MediaType::Other("application/json".into())
            ));
        let run0 = loaded.run(0).expect("run 0 must be reconstructed");
        assert_eq!(run0.run_id(), 0);
        assert!(run0.attachments().iter().any(|attachment| {
            layer_annotation(attachment, ANN_ATTACHMENT_NAME).as_deref() == Some("candidate")
                && attachment.media_type() == &MediaType::Other("application/json".into())
        }));
        let run1 = loaded.run(1).expect("run 1 must be reconstructed");
        assert_eq!(run1.run_id(), 1);
        assert!(run1.attachments().is_empty());

        let mut cells = loaded.run_parameter_cells();
        cells.sort_by(|left, right| (left.run_id, &left.name).cmp(&(right.run_id, &right.name)));
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0].run_id, 0);
        assert_eq!(cells[0].name, "solver");
        assert_eq!(cells[0].value, ParameterValue::String("scip".to_string()));
        assert_eq!(cells[1].run_id, 0);
        assert_eq!(cells[1].name, "time_limit");
        assert_eq!(cells[1].value, ParameterValue::Float(20.0));
        assert_eq!(cells[2].run_id, 1);
        assert_eq!(cells[2].name, "presolve");
        assert_eq!(cells[2].value, ParameterValue::Bool(true));
        assert_eq!(cells[3].run_id, 1);
        assert_eq!(cells[3].name, "solver");
        assert_eq!(cells[3].value, ParameterValue::String("highs".to_string()));
        Ok(())
    });
}

#[test]
fn sealed_experiment_fork_creates_child_with_parent_subject_and_next_run_id() {
    with_temp_experiment(|experiment| {
        experiment.log_json("dataset", json!("miplib2017")).unwrap();
        let instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let solution = instance
            .evaluate(
                &crate::v1::State {
                    entries: HashMap::new(),
                },
                crate::ATol::default(),
            )
            .unwrap();

        {
            let mut run = experiment.run().unwrap();
            run.log_parameter("solver", "base").unwrap();
            run.log_finished_solve_result(
                &instance,
                &solution,
                "dummy.Adapter".to_string(),
                "{}".to_string(),
            )
            .unwrap();
            run.finish().unwrap();
        }

        let parent = experiment.commit().unwrap();
        let parent_artifact = parent.artifact();
        let child_name =
            ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:fork-child").unwrap();
        let child = parent.fork(Name::Named(child_name.clone())).unwrap();
        {
            let mut run = child.run().unwrap();
            assert_eq!(run.run_id(), 1);
            run.log_parameter("solver", "child").unwrap();
            run.finish().unwrap();
        }

        let child = child.commit().unwrap();
        let child_artifact = child.artifact();
        assert_eq!(child_artifact.image_name(), &child_name);
        let subject = child_artifact
            .subject()
            .unwrap()
            .expect("forked child manifest must record parent subject");
        assert_eq!(subject.media_type(), &MediaType::ImageManifest);
        assert_eq!(subject.digest(), parent_artifact.manifest_digest());

        assert!(parent.run(1).is_none());
        let parent_cells = parent.run_parameter_cells();
        assert_eq!(parent_cells.len(), 1);
        assert_eq!(parent_cells[0].run_id, 0);
        assert_eq!(parent_cells[0].name, "solver");
        assert_eq!(
            parent_cells[0].value,
            ParameterValue::String("base".to_string())
        );

        let loaded = SealedExperiment::from_artifact(child_artifact).unwrap();
        assert!(loaded
            .experiment_attachments()
            .iter()
            .any(
                |attachment| layer_annotation(attachment, ANN_ATTACHMENT_NAME).as_deref()
                    == Some("dataset")
            ));
        let run0 = loaded.run(0).unwrap();
        assert_eq!(run0.solves().len(), 1);
        let run1 = loaded.run(1).unwrap();
        assert!(run1.solves().is_empty());
        let mut cells = loaded.run_parameter_cells();
        cells.sort_by_key(|left| left.run_id);
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].run_id, 0);
        assert_eq!(cells[0].value, ParameterValue::String("base".to_string()));
        assert_eq!(cells[1].run_id, 1);
        assert_eq!(cells[1].value, ParameterValue::String("child".to_string()));
        Ok(())
    });
}

#[test]
fn log_finished_solve_result_materializes_solve_entry_with_layer_refs() {
    with_temp_experiment(|experiment| {
        let instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let solution = instance
            .evaluate(
                &crate::v1::State {
                    entries: HashMap::new(),
                },
                crate::ATol::default(),
            )
            .unwrap();

        {
            let mut run = experiment.run().unwrap();
            let solve_id = run
                .log_finished_solve_result(
                    &instance,
                    &solution,
                    "dummy.Adapter".to_string(),
                    r#"{"time_limit":1.5}"#.to_string(),
                )
                .unwrap();
            assert_eq!(solve_id, 0);
            run.finish().unwrap();
        }

        let sealed = experiment.commit().unwrap();
        let artifact = sealed.artifact();
        let layers = artifact.layers().unwrap();
        assert_eq!(layers.len(), 3);

        let config = artifact.stored_config().unwrap();
        let config_json: serde_json::Value =
            serde_json::from_slice(&blob_bytes(&artifact, &config)).unwrap();
        assert_eq!(config_json["attachments"], json!([]));
        assert_eq!(config_json["run_parameters"], json!(2));
        assert_eq!(config_json["runs"][0]["attachments"], json!([]));
        assert_eq!(config_json["runs"][0]["solves"][0]["solve_id"], json!(0));
        assert_eq!(config_json["runs"][0]["solves"][0]["input"], json!(0));
        assert_eq!(config_json["runs"][0]["solves"][0]["output"], json!(1));
        assert_eq!(
            config_json["runs"][0]["solves"][0]["adapter"],
            json!("dummy.Adapter")
        );
        assert_eq!(
            config_json["runs"][0]["solves"][0]["adapter_options"],
            json!(r#"{"time_limit":1.5}"#)
        );

        let loaded = SealedExperiment::from_artifact(artifact.clone()).unwrap();
        let run = loaded.run(0).unwrap();
        assert!(run.attachments().is_empty());
        let solve = &run.solves()[0];
        assert_eq!(solve.solve_id(), 0);
        assert_eq!(solve.input().media_type(), &media_types::v1_instance());
        assert_eq!(solve.output().media_type(), &media_types::v1_solution());
        assert_eq!(
            artifact.get_blob(solve.input()).unwrap(),
            instance.to_bytes()
        );
        assert_eq!(
            artifact.get_blob(solve.output()).unwrap(),
            solution.to_bytes()
        );
        assert_eq!(solve.adapter(), "dummy.Adapter");
        assert_eq!(solve.adapter_options(), r#"{"time_limit":1.5}"#);
        Ok(())
    });
}

#[test]
fn loaded_experiment_rejects_non_finished_status() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = ExperimentConfig {
        status: "crashed".to_string(),
        attachments: Vec::new(),
        runs: Vec::new(),
        run_parameters: LayerRef(0),
    };
    let config_descriptor = registry
        .store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )
        .unwrap();
    let unsealed = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config_descriptor,
        vec![run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:crashed").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("non-finished experiment configs must not load as sealed experiments");
    assert!(err.to_string().contains("status is crashed"));
    assert!(err.to_string().contains(EXPERIMENT_STATUS_FINISHED));
}

#[test]
fn loaded_experiment_rejects_config_attachment_not_listed_in_layers() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), "experiment".to_string());
    annotations.insert(ANN_ATTACHMENT_NAME.to_string(), "outside".to_string());
    let _outside_attachment = registry
        .store_layer_blob(
            MediaType::Other("application/json".to_string()),
            br#""outside""#,
            annotations,
        )
        .unwrap();
    let run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = ExperimentConfig {
        status: EXPERIMENT_STATUS_FINISHED.to_string(),
        attachments: vec![LayerRef(1)],
        runs: Vec::new(),
        run_parameters: LayerRef(0),
    };
    let config_descriptor = registry
        .store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )
        .unwrap();
    let unsealed = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config_descriptor,
        vec![run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:outside-attachment").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("config must not reference attachments outside artifact layers");
    assert!(err
        .to_string()
        .contains("Failed to resolve experiment attachment LayerRef 1"));
}

#[test]
fn loaded_experiment_uses_manifest_layer_metadata_for_attachment_refs() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let mut manifest_annotations = HashMap::new();
    manifest_annotations.insert(ANN_SPACE.to_string(), "experiment".to_string());
    manifest_annotations.insert(ANN_ATTACHMENT_NAME.to_string(), "listed".to_string());
    let listed_attachment = registry
        .store_layer_blob(
            MediaType::Other("application/json".to_string()),
            br#""same-blob""#,
            manifest_annotations,
        )
        .unwrap();

    let run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = ExperimentConfig {
        status: EXPERIMENT_STATUS_FINISHED.to_string(),
        attachments: vec![LayerRef(0)],
        runs: Vec::new(),
        run_parameters: LayerRef(1),
    };
    let config_descriptor = registry
        .store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )
        .unwrap();
    let unsealed = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config_descriptor,
        vec![listed_attachment, run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:unlisted-attachment-metadata")
            .unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let sealed = SealedExperiment::from_artifact(artifact).unwrap();
    assert_eq!(
        layer_annotation(
            &Descriptor::from(sealed.experiment_attachments()[0].clone()),
            ANN_ATTACHMENT_NAME
        )
        .as_deref(),
        Some("listed")
    );
}

#[test]
fn loaded_experiment_rejects_config_run_attachment_not_listed_in_layers() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), "run".to_string());
    annotations.insert(ANN_RUN_ID.to_string(), "0".to_string());
    annotations.insert(ANN_ATTACHMENT_NAME.to_string(), "outside".to_string());
    let _outside_attachment = registry
        .store_layer_blob(
            MediaType::Other("application/json".to_string()),
            br#""outside""#,
            annotations,
        )
        .unwrap();
    let run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = ExperimentConfig {
        status: EXPERIMENT_STATUS_FINISHED.to_string(),
        attachments: Vec::new(),
        runs: vec![ExperimentConfigRun {
            run_id: 0,
            attachments: vec![LayerRef(1)],
            solves: Vec::new(),
        }],
        run_parameters: LayerRef(0),
    };
    let config_descriptor = registry
        .store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )
        .unwrap();
    let unsealed = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config_descriptor,
        vec![run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:outside-run-attachment").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("config must not reference run attachments outside artifact layers");
    assert!(err
        .to_string()
        .contains("Failed to resolve run 0 attachment LayerRef 1"));
}

#[test]
fn loaded_experiment_rejects_run_parameters_not_listed_in_layers() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let _run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = ExperimentConfig {
        status: EXPERIMENT_STATUS_FINISHED.to_string(),
        attachments: Vec::new(),
        runs: Vec::new(),
        run_parameters: LayerRef(0),
    };
    let config_descriptor = registry
        .store_json_blob(
            MediaType::Other(EXPERIMENT_CONFIG_MEDIA_TYPE.to_string()),
            &config,
        )
        .unwrap();
    let unsealed = UnsealedArtifact::new(
        MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string()),
        config_descriptor,
        Vec::new(),
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:outside-run-parameters").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("config must not reference run parameters outside artifact layers");
    assert!(err
        .to_string()
        .contains("Failed to resolve run-parameter table LayerRef 0"));
}

#[test]
fn log_parameter_rejects_non_finite_float_values() {
    with_temp_experiment(|experiment| {
        let mut run = experiment.run().unwrap();

        let err = run
            .log_parameter("time_limit", f64::NAN)
            .expect_err("parameter table accepts only finite float values");
        assert!(err.to_string().contains("must be finite"));
        Ok(())
    });
}

#[test]
fn log_parameter_promotes_int_column_to_float_at_commit() {
    with_temp_experiment(|experiment| {
        {
            let mut run0 = experiment.run().unwrap();
            run0.log_parameter("time_limit", 10).unwrap();
            run0.finish().unwrap();
        }
        {
            let mut run1 = experiment.run().unwrap();
            run1.log_parameter("time_limit", 20.5).unwrap();
            run1.finish().unwrap();
        }

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        let run_params = find_layer(&layers, ANN_LAYER, LAYER_KIND_RUN_PARAMETERS);
        let bytes = blob_bytes(&artifact, run_params);
        let table: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(
            table,
            json!({
                "columns": {
                    "time_limit": {
                        "type": "float64",
                        "values": {
                            "0": 10.0,
                            "1": 20.5,
                        },
                    },
                },
            })
        );
        Ok(())
    });
}

#[test]
fn commit_rejects_mixed_parameter_column_types() {
    with_temp_experiment(|experiment| {
        {
            let mut run0 = experiment.run().unwrap();
            run0.log_parameter("seed", 1).unwrap();
            run0.finish().unwrap();
        }
        {
            let mut run1 = experiment.run().unwrap();
            run1.log_parameter("seed", "1").unwrap();
            run1.finish().unwrap();
        }

        let err = experiment
            .commit()
            .expect_err("mixed parameter column types must be rejected");
        assert!(err.to_string().contains("mixed column types"));
        Ok(())
    });
}

/// `commit()` consumes the unsealed session and returns a sealed handle.
/// Further `log_*` / `run()` calls on the original session are impossible
/// in Rust because the original `Experiment` value has moved.
#[test]
fn commit_returns_sealed_experiment() {
    with_temp_experiment(|experiment| {
        {
            let mut run = experiment.run().unwrap();
            run.log_json("seed", json!(0)).unwrap();
            run.finish().unwrap();
        }

        let sealed = experiment.commit().unwrap();
        let artifact = sealed.artifact();
        let config = artifact.stored_config().unwrap();
        let config_json: serde_json::Value =
            serde_json::from_slice(&blob_bytes(&artifact, &config)).unwrap();
        assert_eq!(
            config_json.get("status").and_then(|value| value.as_str()),
            Some(EXPERIMENT_STATUS_FINISHED)
        );
        Ok(())
    });
}

#[test]
fn anonymous_experiment_uses_registry_generated_image_name() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let experiment = Experiment::with_registry(temp.registry(), Name::Anonymous).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();

    let artifact = experiment.commit().unwrap().into_artifact();
    let image_name = artifact.image_name();
    let repository_key = image_name.repository_key();
    let host = repository_key
        .strip_suffix(".ommx.local/experiment")
        .expect("anonymous experiment uses the experiment repository");
    assert_eq!(host.len(), 8);
    assert!(host
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    assert!(crate::artifact::is_anonymous_artifact_tag(
        image_name.reference()
    ));
    assert!(temp
        .registry()
        .resolve_image_name(image_name)
        .unwrap()
        .is_some());
}

#[test]
fn named_experiment_uses_requested_image_name() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let image_name =
        crate::artifact::ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:requested-name")
            .unwrap();
    let experiment =
        Experiment::with_registry(temp.registry(), Name::Named(image_name.clone())).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();

    let artifact = experiment.commit().unwrap().into_artifact();
    assert_eq!(artifact.image_name(), &image_name);
    assert!(temp
        .registry()
        .resolve_image_name(&image_name)
        .unwrap()
        .is_some());
}

/// Dropping a run handle without closing it does not write its local
/// state back to the experiment. BlobStore payloads written before the
/// drop may remain as orphan blobs until GC.
#[test]
fn dropping_unclosed_run_does_not_write_back() {
    with_temp_experiment(|experiment| {
        {
            let mut run = experiment.run().unwrap();
            run.log_json("seed", json!(0)).unwrap();
        }

        assert_eq!(
            with_unsealed_state(&experiment, |state| state.runs.len()),
            0
        );
        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        assert!(layers
            .iter()
            .all(|layer| layer_annotation(layer, ANN_ATTACHMENT_NAME).as_deref() != Some("seed")));
        Ok(())
    });
}

/// A byte-identical attachment logged by two runs yields two annotation-
/// distinct layer descriptors backed by one shared CAS blob.
#[test]
fn byte_identical_attachment_across_runs_shares_one_blob() {
    with_temp_experiment(|experiment| {
        let payload = json!({ "formulation": "relaxed" });

        {
            let mut run0 = experiment.run().unwrap();
            run0.log_json("candidate", payload.clone()).unwrap();
            run0.finish().unwrap();
        }

        {
            let mut run1 = experiment.run().unwrap();
            run1.log_json("candidate", payload.clone()).unwrap();
            run1.finish().unwrap();
        }

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();

        let candidates: Vec<&StoredDescriptor<'_>> = layers
            .iter()
            .filter(|layer| {
                layer_annotation(layer, ANN_ATTACHMENT_NAME).as_deref() == Some("candidate")
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
        Ok(())
    });
}

/// Caller-defined payload types are represented directly by OCI media
/// type, without an additional OMMX attachment-kind axis.
#[test]
fn log_attachment_accepts_caller_defined_media_type() {
    with_temp_experiment(|experiment| {
        let media_type = MediaType::Other("application/vnd.jijmodeling.model+json".to_string());
        experiment
            .log_attachment("source-model", media_type.clone(), br#"{"variables": []}"#)
            .unwrap();

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        let source_model = find_layer(&layers, ANN_ATTACHMENT_NAME, "source-model");
        assert_eq!(source_model.media_type(), &media_type);
        assert_eq!(blob_bytes(&artifact, source_model), br#"{"variables": []}"#);
        Ok(())
    });
}

#[test]
fn experiment_dyn_keeps_temp_registry_alive_for_derived_artifacts() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish().unwrap();
    }

    let artifact = experiment.commit().unwrap();
    drop(experiment);

    let loaded = ExperimentDyn::from_artifact(artifact).unwrap();
    let cells = loaded.run_parameter_cells().unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].run_id, 0);
    assert_eq!(cells[0].name, "solver");
    assert_eq!(cells[0].value, ParameterValue::String("scip".to_string()));
}

#[test]
fn experiment_dyn_rejects_commit_while_run_is_open() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    let run = experiment.run().unwrap();

    let err = experiment
        .commit()
        .expect_err("open RunDyn must block commit");
    assert!(err.to_string().contains("Run handle"));

    run.abandon();
    experiment.commit().unwrap();
}

#[test]
fn experiment_dyn_rejects_second_commit_as_sealed() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    experiment.commit().unwrap();

    let err = experiment
        .commit()
        .expect_err("sealed Experiment must reject a second commit");
    assert!(err.to_string().contains("read-only"));
    assert_eq!(experiment.state_name(), "sealed");
}

#[test]
fn experiment_dyn_drops_unfinished_run_as_abandoned() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
    }

    experiment.commit().unwrap();
    assert!(experiment.run_parameter_cells().unwrap().is_empty());
}

#[test]
fn experiment_dyn_marks_commit_failure_explicitly() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let image_name = ImageRef::parse("example.com/ommx/conflict:latest").unwrap();

    ExperimentDyn::with_registry_handle(registry_handle.clone(), image_name.clone())
        .unwrap()
        .commit()
        .unwrap();

    let experiment = ExperimentDyn::with_registry_handle(registry_handle, image_name).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish().unwrap();
    }
    let err = experiment
        .commit()
        .expect_err("publishing the same ref must conflict");
    assert!(err.to_string().contains("already points"));
    assert_eq!(experiment.state_name(), "failed");
    assert_eq!(experiment.open_run_count(), 0);

    let err = experiment
        .commit()
        .expect_err("failed Experiment must report the stored failure reason");
    assert!(err.to_string().contains("commit has failed"));
    assert!(err.to_string().contains("already points"));

    let err = experiment
        .run()
        .expect_err("failed Experiment must reject new runs");
    assert!(err.to_string().contains("commit has failed"));
}

#[test]
fn experiment_dyn_rename_before_commit_changes_publish_ref() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), Name::Anonymous).unwrap();
    let old_image_name = experiment.image_name().unwrap();
    let new_image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/renamed-before:latest").unwrap();

    experiment.rename(new_image_name.clone()).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    let artifact = experiment.commit().unwrap();

    assert_eq!(artifact.image_name(), &new_image_name);
    assert_eq!(experiment.image_name().unwrap(), new_image_name);
    assert!(registry_handle
        .registry()
        .resolve_image_name(&old_image_name)
        .unwrap()
        .is_none());
    assert!(registry_handle
        .registry()
        .resolve_image_name(artifact.image_name())
        .unwrap()
        .is_some());
}

#[test]
fn experiment_dyn_rename_after_commit_publishes_alias() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), Name::Anonymous).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    let artifact = experiment.commit().unwrap();
    let old_image_name = artifact.image_name().clone();
    let old_digest = registry_handle
        .registry()
        .resolve_image_name(&old_image_name)
        .unwrap()
        .unwrap();
    let new_image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/renamed-after:latest").unwrap();

    experiment.rename(new_image_name.clone()).unwrap();
    let new_digest = registry_handle
        .registry()
        .resolve_image_name(&new_image_name)
        .unwrap()
        .unwrap();

    assert_eq!(old_digest, new_digest);
    assert_eq!(experiment.image_name().unwrap(), new_image_name);
    assert_eq!(experiment.artifact().unwrap().image_name(), &new_image_name);
    assert!(registry_handle
        .registry()
        .resolve_image_name(&old_image_name)
        .unwrap()
        .is_some());
}

#[test]
fn experiment_dyn_save_writes_committed_archive() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    let artifact = experiment.commit().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("experiment.ommx");

    experiment.save(&archive_path).unwrap();

    assert!(archive_path.exists());
    assert!(archive_path.metadata().unwrap().len() > 0);
    assert_eq!(
        experiment.artifact().unwrap().image_name(),
        artifact.image_name()
    );

    let loaded = ExperimentDyn::import_archive(&archive_path).unwrap();
    assert_eq!(loaded.experiment_attachments().unwrap().len(), 1);
}

#[cfg(feature = "remote-artifact")]
#[test]
fn experiment_dyn_push_rejects_uncommitted_experiment() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();

    let err = experiment
        .push()
        .expect_err("uncommitted Experiment must reject push");

    assert!(err.to_string().contains("must be committed"));
}
