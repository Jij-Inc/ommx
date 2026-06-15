//! Tests for the experiment session model.

use super::config::{ExperimentConfig, ExperimentConfigRun, ExperimentConfigSolve, LayerRef};
use super::UnsealedExperimentState;
use super::{
    AttachmentLogger, AttachmentTable, Experiment, ExperimentDyn, ExperimentStatus, Name,
    ParameterValue, SealedExperiment, SolveDiagnosticPayload, SolveStatus, Trace,
    EXPERIMENT_CONFIG_MEDIA_TYPE, EXPERIMENT_STATUS_DRAFT, EXPERIMENT_STATUS_FAILED,
    EXPERIMENT_STATUS_FINISHED, EXPERIMENT_STATUS_INTERRUPTED, RUN_PARAMETERS_MEDIA_TYPE,
    RUN_STATUS_FAILED, RUN_STATUS_FINISHED, RUN_STATUS_INTERRUPTED,
};
use crate::artifact::local_registry::{StoredDescriptor, UnsealedArtifact};
use crate::artifact::{
    media_types, sha256_digest, AsArtifact, ImageRef, LocalArtifact, LocalArtifactDyn,
    LocalRegistryHandle,
};
use crate::{Coefficient, Evaluate, Function, Instance, Sense};
use oci_spec::image::{Digest, MediaType};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::{fs, str::FromStr};

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

#[test]
fn file_attachment_infers_media_type_from_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.png");
    let bytes = b"\x89PNG\r\n\x1a\n";
    fs::write(&path, bytes).unwrap();

    with_temp_experiment(|experiment| {
        AttachmentLogger::log_file(&experiment, "source-file", &path, None, None).unwrap();

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        let config = experiment_config(&artifact);
        let source_file = layer_from_ref(&layers, *config.attachments.get("source-file").unwrap());

        assert_eq!(source_file.media_type().to_string(), "image/png");
        assert_eq!(blob_bytes(&artifact, source_file), bytes);
        assert_eq!(
            config.attachments.filename("source-file"),
            Some("source.png")
        );
        Ok(())
    });
}

fn layer_from_ref<'a, 'reg>(
    layers: &'a [StoredDescriptor<'reg>],
    layer_ref: LayerRef,
) -> &'a StoredDescriptor<'reg> {
    layers
        .get(layer_ref.0 as usize)
        .unwrap_or_else(|| panic!("LayerRef {} is out of bounds", layer_ref.0))
}

fn experiment_config(artifact: &LocalArtifact<'_>) -> ExperimentConfig {
    let config = artifact.stored_config().unwrap();
    serde_json::from_slice(&blob_bytes(artifact, &config)).unwrap()
}

fn blob_bytes(artifact: &LocalArtifact<'_>, descriptor: &StoredDescriptor<'_>) -> Vec<u8> {
    artifact.get_blob(descriptor).unwrap()
}

fn digest_for_bytes(bytes: &[u8]) -> Digest {
    Digest::from_str(&sha256_digest(bytes)).unwrap()
}

fn assert_blob_absent(experiment: &Experiment<'_>, bytes: &[u8]) {
    let digest = digest_for_bytes(bytes);
    assert!(
        !experiment.registry.contains_blob(&digest).unwrap(),
        "unexpected blob was stored: {digest}"
    );
}

fn constant_instance(sense: Sense, objective: f64) -> Instance {
    Instance::new(
        sense,
        Function::Constant(Coefficient::try_from(objective).unwrap()),
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .unwrap()
}

fn empty_solution(instance: &Instance) -> crate::Solution {
    instance
        .evaluate(
            &crate::v1::State {
                entries: HashMap::new(),
            },
            crate::ATol::default(),
        )
        .unwrap()
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
        let config = experiment_config(&artifact);
        assert_eq!(
            config
                .runs
                .iter()
                .filter(|run| run.attachments.contains_key("candidate"))
                .map(|run| run.run_id)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        Ok(())
    });
}

/// `log_*` writes the payload to the Local Registry immediately, before any
/// commit advances a public ref.
#[test]
fn log_writes_blob_to_registry_immediately() {
    with_temp_experiment(|experiment| {
        {
            let mut run = experiment.run().unwrap();
            run.log_json("solver", json!("scip")).unwrap();
            run.finish().unwrap();
        }

        let digest = with_unsealed_state(&experiment, |state| {
            let run = state.runs.get(&0).unwrap();
            assert_eq!(run.attachments.len(), 1);
            run.attachments.get("solver").unwrap().digest().clone()
        });
        assert!(experiment.registry.contains_blob(&digest).unwrap());
        Ok(())
    });
}

#[test]
fn trace_is_config_referenced_manifest_layer() {
    with_temp_experiment(|experiment| {
        let mut run = experiment.run().unwrap();
        run.store_trace(Trace::from_bytes(b"trace".to_vec()))
            .unwrap();
        run.finish().unwrap();

        let artifact = experiment.commit().unwrap().into_artifact();
        let layers = artifact.layers().unwrap();
        let config = experiment_config(&artifact);
        let trace_ref = config.runs[0].trace.expect("run has a trace ref");
        let trace = layer_from_ref(&layers, trace_ref);
        assert_eq!(trace.media_type(), &media_types::trace_otlp_protobuf());
        assert!(trace.annotations().as_ref().is_none_or(HashMap::is_empty));
        let loaded = SealedExperiment::from_artifact(artifact).unwrap();
        assert_eq!(loaded.status(), &ExperimentStatus::Finished);
        assert!(loaded.run(0).unwrap().trace().unwrap().is_some());
        Ok(())
    });
}

#[test]
fn duplicate_attachment_names_are_rejected_per_namespace() {
    with_temp_experiment(|experiment| {
        experiment.log_json("dataset", json!("first")).unwrap();
        let err = experiment
            .log_json("dataset", json!("second"))
            .expect_err("duplicate experiment attachment names must be rejected");
        assert!(err.to_string().contains("already exists"));

        {
            let mut run = experiment.run().unwrap();
            run.log_json("candidate", json!("first")).unwrap();
            let err = run
                .log_json("candidate", json!("second"))
                .expect_err("duplicate run attachment names must be rejected");
            assert!(err.to_string().contains("already exists"));
            run.finish().unwrap();
        }

        {
            let mut run = experiment.run().unwrap();
            run.log_json("candidate", json!("same name in another run"))
                .unwrap();
            run.finish().unwrap();
        }

        let loaded = experiment.commit().unwrap();
        assert!(loaded.contains_attachment("dataset"));
        assert!(loaded.run(0).unwrap().contains_attachment("candidate"));
        assert!(loaded.run(1).unwrap().contains_attachment("candidate"));
        Ok(())
    });
}

#[test]
fn run_rejects_second_trace() {
    with_temp_experiment(|experiment| {
        let mut run = experiment.run().unwrap();
        run.store_trace(Trace::from_bytes(b"trace-1".to_vec()))
            .unwrap();

        let err = run
            .store_trace(Trace::from_bytes(b"trace-2".to_vec()))
            .expect_err("a Run can store at most one trace");

        assert!(err.to_string().contains("already has a trace"));
        Ok(())
    });
}

#[test]
fn file_attachment_filename_is_stored_in_config_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.json");
    fs::write(&path, br#"{"ok":true}"#).unwrap();

    with_temp_experiment(|experiment| {
        AttachmentLogger::log_file(
            &experiment,
            "source-file",
            &path,
            Some(MediaType::from("application/json")),
            Some("input.json"),
        )
        .unwrap();

        let artifact = experiment.commit().unwrap().into_artifact();
        let config = experiment_config(&artifact);
        assert_eq!(
            config.attachments.filename("source-file"),
            Some("input.json")
        );
        Ok(())
    });
}

#[test]
fn log_json_encodes_hash_maps_stably() {
    fn map(entries: impl IntoIterator<Item = (&'static str, i32)>) -> HashMap<&'static str, i32> {
        entries.into_iter().collect()
    }

    let first = HashMap::from([
        ("outer_b", map([("d", 4), ("c", 3)])),
        ("outer_a", map([("b", 2), ("a", 1)])),
    ]);
    let second = HashMap::from([
        ("outer_a", map([("a", 1), ("b", 2)])),
        ("outer_b", map([("c", 3), ("d", 4)])),
    ]);

    with_temp_experiment(|experiment| {
        experiment.log_json("first", first).unwrap();
        experiment.log_json("second", second).unwrap();

        let bytes = with_unsealed_state(&experiment, |state| {
            state
                .attachments
                .names()
                .map(|name| state.attachments.get(name).unwrap())
                .map(|descriptor| experiment.registry.read_blob(descriptor.digest()).unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(bytes.len(), 2);
        assert_eq!(bytes[0], bytes[1]);
        assert_eq!(
            bytes[0],
            br#"{"outer_a":{"a":1,"b":2},"outer_b":{"c":3,"d":4}}"#
        );
        Ok(())
    });
}

/// `commit()` seals the session into an OMMX Artifact whose config describes
/// the experiment / run attachments.
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
        let config: ExperimentConfig =
            serde_json::from_slice(&blob_bytes(&artifact, &config)).unwrap();
        let config_json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            config_json.get("status").and_then(|value| value.as_str()),
            Some(EXPERIMENT_STATUS_FINISHED)
        );

        // 3 attachments (1 experiment-space + 2 run-space) + run-parameters.
        let layers = artifact.layers().unwrap();
        assert_eq!(layers.len(), 4);

        let dataset = layer_from_ref(&layers, *config.attachments.get("dataset").unwrap());
        assert_eq!(
            dataset.media_type(),
            &MediaType::Other("application/json".into())
        );
        assert!(dataset.annotations().as_ref().is_none_or(HashMap::is_empty));

        let run = &config.runs[0];
        let candidate = layer_from_ref(&layers, *run.attachments.get("candidate").unwrap());
        assert_eq!(candidate.media_type(), &media_types::v1_instance());
        let candidate_annotations = candidate
            .annotations()
            .as_ref()
            .expect("instance layer should mirror protobuf metadata");
        assert_eq!(
            candidate_annotations.get("org.ommx.v1.instance.variables"),
            Some(&instance.decision_variables().len().to_string())
        );
        assert_eq!(
            candidate_annotations.get("org.ommx.v1.instance.constraints"),
            Some(&instance.constraints().len().to_string())
        );
        assert_eq!(blob_bytes(&artifact, candidate), instance.to_bytes());

        // Aggregate layers are not tagged as attachments.
        let run_params = layer_from_ref(&layers, config.run_parameters);
        assert_eq!(
            run_params.media_type(),
            &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string())
        );
        assert!(run_params
            .annotations()
            .as_ref()
            .is_none_or(HashMap::is_empty));

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
        let config = experiment_config(&artifact);
        let layers = artifact.layers().unwrap();
        let run_params = layer_from_ref(&layers, config.run_parameters);
        assert_eq!(
            run_params.media_type(),
            &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string())
        );
        assert!(run_params
            .annotations()
            .as_ref()
            .is_none_or(HashMap::is_empty));
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

        assert_eq!(
            loaded.attachment_media_type("dataset").unwrap(),
            MediaType::Other("application/json".into())
        );
        let run0 = loaded.run(0).expect("run 0 must be reconstructed");
        assert_eq!(run0.run_id(), 0);
        assert_eq!(
            run0.attachment_media_type("candidate").unwrap(),
            MediaType::Other("application/json".into())
        );
        let run1 = loaded.run(1).expect("run 1 must be reconstructed");
        assert_eq!(run1.run_id(), 1);
        assert_eq!(run1.attachment_names().count(), 0);

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
            run.log_finished_solve(super::FinishedSolveRecord {
                input: &instance,
                output: &solution,
                adapter: "dummy.Adapter".to_string(),
                adapter_options: "{}".to_string(),
                diagnostics: None,
            })
            .unwrap();
            run.store_trace(Trace::from_bytes(b"parent trace".to_vec()))
                .unwrap();
            run.finish().unwrap();
        }

        let parent = experiment.commit().unwrap();
        let parent_artifact = parent.artifact();
        let parent_trace_digest = parent
            .run(0)
            .unwrap()
            .trace_descriptor()
            .expect("parent run has trace")
            .digest()
            .clone();
        let child_name =
            ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:fork-child").unwrap();
        let child = parent.fork(Name::Named(child_name.clone())).unwrap();
        {
            let mut run = child.run().unwrap();
            assert_eq!(run.run_id(), 1);
            run.log_parameter("solver", "child").unwrap();
            run.store_trace(Trace::from_bytes(b"child trace".to_vec()))
                .unwrap();
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
        assert_eq!(
            loaded.run(0).unwrap().trace_descriptor().unwrap().digest(),
            &parent_trace_digest
        );
        assert!(loaded.run(1).unwrap().trace().unwrap().is_some());
        assert!(loaded.contains_attachment("dataset"));
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
fn log_finished_solve_materializes_solve_entry_with_layer_refs() {
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
        let diagnostics = b"\x91\x81\xa6status\xa7optimal".to_vec();

        {
            let mut run = experiment.run().unwrap();
            let solve_id = run
                .log_finished_solve(super::FinishedSolveRecord {
                    input: &instance,
                    output: &solution,
                    adapter: "dummy.Adapter".to_string(),
                    adapter_options: r#"{"time_limit":1.5}"#.to_string(),
                    diagnostics: Some(SolveDiagnosticPayload::new(diagnostics.clone())?),
                })
                .unwrap();
            assert_eq!(solve_id, 0);
            run.finish().unwrap();
        }

        let sealed = experiment.commit().unwrap();
        let artifact = sealed.artifact();
        let layers = artifact.layers().unwrap();
        assert_eq!(layers.len(), 4);

        let config = artifact.stored_config().unwrap();
        let config_json: serde_json::Value =
            serde_json::from_slice(&blob_bytes(&artifact, &config)).unwrap();
        assert_eq!(config_json["attachments"], json!({ "entries": {} }));
        assert_eq!(config_json["run_parameters"], json!(3));
        assert_eq!(config_json["runs"][0]["status"], json!(RUN_STATUS_FINISHED));
        assert_eq!(
            config_json["runs"][0]["attachments"],
            json!({ "entries": {} })
        );
        assert_eq!(config_json["runs"][0]["solves"][0]["solve_id"], json!(0));
        assert_eq!(config_json["runs"][0]["solves"][0]["input"], json!(0));
        assert_eq!(config_json["runs"][0]["solves"][0]["output"], json!(1));
        assert_eq!(config_json["runs"][0]["solves"][0]["diagnostics"], json!(2));
        let diagnostic_layer = layer_from_ref(&layers, LayerRef(2));
        assert_eq!(
            diagnostic_layer.media_type(),
            &media_types::diagnostic_msgpack()
        );
        assert!(diagnostic_layer
            .annotations()
            .as_ref()
            .is_none_or(HashMap::is_empty));
        assert_eq!(blob_bytes(&artifact, diagnostic_layer), diagnostics);
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
        assert_eq!(run.attachment_names().count(), 0);
        let solve = &run.solves()[0];
        assert_eq!(solve.solve_id(), 0);
        assert_eq!(
            solve.input_instance().unwrap().to_bytes(),
            instance.to_bytes()
        );
        assert_eq!(
            solve.output_solution().unwrap().unwrap().to_bytes(),
            solution.to_bytes()
        );
        assert_eq!(solve.adapter(), "dummy.Adapter");
        assert_eq!(solve.adapter_options(), r#"{"time_limit":1.5}"#);
        assert_eq!(
            solve.diagnostic_blob().unwrap().as_deref(),
            Some(&*diagnostics)
        );
        let diagnostic_payload = solve.diagnostic_payload().unwrap().unwrap();
        assert!(matches!(
            diagnostic_payload.value(),
            rmpv::Value::Array(items) if items.len() == 1
        ));
        Ok(())
    });
}

#[test]
fn log_finished_solve_with_id_validates_id_before_storing_payloads() {
    with_temp_experiment(|experiment| {
        let unreserved_instance = constant_instance(Sense::Minimize, 10.0);
        let unreserved_solution = empty_solution(&unreserved_instance);
        let unreserved_diagnostics = SolveDiagnosticPayload::new(vec![0x91, 0x01])?;
        let unreserved_input_bytes = unreserved_instance.to_bytes();
        let unreserved_output_bytes = unreserved_solution.to_bytes();
        let unreserved_diagnostic_bytes = unreserved_diagnostics.to_msgpack_bytes()?;

        {
            let mut run = experiment.run().unwrap();
            let err = run
                .log_finished_solve_with_id(
                    0,
                    super::FinishedSolveRecord {
                        input: &unreserved_instance,
                        output: &unreserved_solution,
                        adapter: "dummy.Adapter".to_string(),
                        adapter_options: "{}".to_string(),
                        diagnostics: Some(unreserved_diagnostics),
                    },
                )
                .expect_err("unreserved solve ID must be rejected before payload storage");
            assert!(err.to_string().contains("has not been reserved"), "{err:#}");
            run.finish().unwrap();
        }
        assert_blob_absent(&experiment, &unreserved_input_bytes);
        assert_blob_absent(&experiment, &unreserved_output_bytes);
        assert_blob_absent(&experiment, &unreserved_diagnostic_bytes);

        let first_instance = constant_instance(Sense::Minimize, 20.0);
        let first_solution = empty_solution(&first_instance);
        let duplicate_instance = constant_instance(Sense::Maximize, 30.0);
        let duplicate_solution = empty_solution(&duplicate_instance);
        let duplicate_diagnostics = SolveDiagnosticPayload::new(vec![0x91, 0x02])?;
        let duplicate_input_bytes = duplicate_instance.to_bytes();
        let duplicate_output_bytes = duplicate_solution.to_bytes();
        let duplicate_diagnostic_bytes = duplicate_diagnostics.to_msgpack_bytes()?;

        {
            let mut run = experiment.run().unwrap();
            let solve_id = run.reserve_solve_id();
            run.log_finished_solve_with_id(
                solve_id,
                super::FinishedSolveRecord {
                    input: &first_instance,
                    output: &first_solution,
                    adapter: "dummy.Adapter".to_string(),
                    adapter_options: "{}".to_string(),
                    diagnostics: None,
                },
            )
            .unwrap();
            let err = run
                .log_finished_solve_with_id(
                    solve_id,
                    super::FinishedSolveRecord {
                        input: &duplicate_instance,
                        output: &duplicate_solution,
                        adapter: "dummy.Adapter".to_string(),
                        adapter_options: "{}".to_string(),
                        diagnostics: Some(duplicate_diagnostics),
                    },
                )
                .expect_err("duplicate solve ID must be rejected before payload storage");
            assert!(
                err.to_string().contains("already contains Solve"),
                "{err:#}"
            );
            run.finish().unwrap();
        }
        assert_blob_absent(&experiment, &duplicate_input_bytes);
        assert_blob_absent(&experiment, &duplicate_output_bytes);
        assert_blob_absent(&experiment, &duplicate_diagnostic_bytes);
        Ok(())
    });
}

#[test]
fn log_failed_solve_with_id_validates_id_before_storing_payloads() {
    with_temp_experiment(|experiment| {
        let unreserved_instance = constant_instance(Sense::Minimize, 40.0);
        let unreserved_diagnostics = SolveDiagnosticPayload::new(vec![0x91, 0x03])?;
        let unreserved_input_bytes = unreserved_instance.to_bytes();
        let unreserved_diagnostic_bytes = unreserved_diagnostics.to_msgpack_bytes()?;

        {
            let mut run = experiment.run().unwrap();
            let err = run
                .log_failed_solve_with_id(
                    0,
                    super::FailedSolveRecord {
                        input: &unreserved_instance,
                        adapter: "dummy.Adapter".to_string(),
                        adapter_options: "{}".to_string(),
                        status: SolveStatus::Failed,
                        diagnostics: Some(unreserved_diagnostics),
                    },
                )
                .expect_err("unreserved solve ID must be rejected before payload storage");
            assert!(err.to_string().contains("has not been reserved"), "{err:#}");
            run.finish().unwrap();
        }
        assert_blob_absent(&experiment, &unreserved_input_bytes);
        assert_blob_absent(&experiment, &unreserved_diagnostic_bytes);

        let first_instance = constant_instance(Sense::Minimize, 50.0);
        let duplicate_instance = constant_instance(Sense::Maximize, 60.0);
        let duplicate_diagnostics = SolveDiagnosticPayload::new(vec![0x91, 0x04])?;
        let duplicate_input_bytes = duplicate_instance.to_bytes();
        let duplicate_diagnostic_bytes = duplicate_diagnostics.to_msgpack_bytes()?;

        {
            let mut run = experiment.run().unwrap();
            let solve_id = run.reserve_solve_id();
            run.log_failed_solve_with_id(
                solve_id,
                super::FailedSolveRecord {
                    input: &first_instance,
                    adapter: "dummy.Adapter".to_string(),
                    adapter_options: "{}".to_string(),
                    status: SolveStatus::Failed,
                    diagnostics: None,
                },
            )
            .unwrap();
            let err = run
                .log_failed_solve_with_id(
                    solve_id,
                    super::FailedSolveRecord {
                        input: &duplicate_instance,
                        adapter: "dummy.Adapter".to_string(),
                        adapter_options: "{}".to_string(),
                        status: SolveStatus::Interrupted,
                        diagnostics: Some(duplicate_diagnostics),
                    },
                )
                .expect_err("duplicate solve ID must be rejected before payload storage");
            assert!(
                err.to_string().contains("already contains Solve"),
                "{err:#}"
            );
            run.finish().unwrap();
        }
        assert_blob_absent(&experiment, &duplicate_input_bytes);
        assert_blob_absent(&experiment, &duplicate_diagnostic_bytes);
        Ok(())
    });
}

#[test]
fn solve_diagnostic_payload_requires_messagepack_array() {
    let err = SolveDiagnosticPayload::new(vec![0xc4, 0x01])
        .expect_err("diagnostic payload must be valid MessagePack");
    assert!(err.to_string().contains("valid MessagePack"), "{err:#}");

    let map_payload = b"\x81\xa6status\xa7optimal".to_vec();
    let err = SolveDiagnosticPayload::new(map_payload)
        .expect_err("diagnostic payload must be a top-level array");
    assert!(err.to_string().contains("MessagePack array"), "{err:#}");

    let trailing_payload = b"\x90\x00".to_vec();
    let err = SolveDiagnosticPayload::new(trailing_payload)
        .expect_err("diagnostic payload must contain exactly one value");
    assert!(
        err.to_string().contains("exactly one MessagePack value"),
        "{err:#}"
    );
}

#[test]
fn loaded_experiment_rejects_invalid_diagnostic_payload() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let input = registry
        .store_layer_blob(media_types::v1_instance(), b"input", HashMap::new())
        .unwrap();
    let output = registry
        .store_layer_blob(media_types::v1_solution(), b"output", HashMap::new())
        .unwrap();
    let diagnostics = registry
        .store_layer_blob(
            media_types::diagnostic_msgpack(),
            b"\x81\xa6status\xa7optimal",
            HashMap::new(),
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
        requested_image_name: None,
        attachments: AttachmentTable::new(),
        runs: vec![ExperimentConfigRun {
            run_id: 0,
            status: RUN_STATUS_FINISHED.to_string(),
            attachments: AttachmentTable::new(),
            trace: None,
            solves: vec![ExperimentConfigSolve {
                solve_id: 0,
                status: super::SOLVE_STATUS_FINISHED.to_string(),
                input: LayerRef(0),
                output: Some(LayerRef(1)),
                adapter: "dummy.Adapter".to_string(),
                adapter_options: "{}".to_string(),
                diagnostics: Some(LayerRef(2)),
            }],
        }],
        run_parameters: LayerRef(3),
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
        vec![input, output, diagnostics, run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:invalid-diagnostics").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("diagnostic payload must be validated when loading an artifact");
    let messages = err
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("Invalid Run 0 Solve 0 diagnostic payload"));
    assert!(messages.contains("MessagePack array"));
}

#[test]
fn loaded_experiment_rejects_failed_solve_with_output() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let input = registry
        .store_layer_blob(media_types::v1_instance(), b"input", HashMap::new())
        .unwrap();
    let output = registry
        .store_layer_blob(media_types::v1_solution(), b"output", HashMap::new())
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
        requested_image_name: None,
        attachments: AttachmentTable::new(),
        runs: vec![ExperimentConfigRun {
            run_id: 0,
            status: RUN_STATUS_FINISHED.to_string(),
            attachments: AttachmentTable::new(),
            trace: None,
            solves: vec![ExperimentConfigSolve {
                solve_id: 0,
                status: super::SOLVE_STATUS_FAILED.to_string(),
                input: LayerRef(0),
                output: Some(LayerRef(1)),
                adapter: "dummy.Adapter".to_string(),
                adapter_options: "{}".to_string(),
                diagnostics: None,
            }],
        }],
        run_parameters: LayerRef(2),
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
        vec![input, output, run_parameters],
        None,
        HashMap::new(),
    );
    let sealed_artifact = registry.seal_artifact(unsealed).unwrap();
    let image_name =
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:failed-solve-output").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("failed Solve configs must not carry output layers");
    assert!(err
        .to_string()
        .contains("Run 0 Solve 0 has status failed but has an output"));
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
        requested_image_name: None,
        attachments: AttachmentTable::new(),
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
    let _outside_attachment = registry
        .store_layer_blob(
            MediaType::Other("application/json".to_string()),
            br#""outside""#,
            HashMap::new(),
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
        requested_image_name: None,
        attachments: AttachmentTable::from_entries([("outside", LayerRef(1))]).unwrap(),
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
        .contains("Failed to resolve experiment attachment `outside` LayerRef 1"));
}

#[test]
fn loaded_experiment_uses_config_table_for_attachment_names() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let mut manifest_annotations = HashMap::new();
    manifest_annotations.insert(
        "org.ommx.user.attachment_name".to_string(),
        "descriptor-name".to_string(),
    );
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
        requested_image_name: None,
        attachments: AttachmentTable::from_entries([("config-name", LayerRef(0))]).unwrap(),
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

    let layers = artifact.layers().unwrap();
    let sealed = SealedExperiment::from_artifact(artifact).unwrap();
    assert!(sealed.contains_attachment("config-name"));
    assert_eq!(
        layer_from_ref(&layers, LayerRef(0))
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get("org.ommx.user.attachment_name"))
            .map(String::as_str),
        Some("descriptor-name")
    );
}

#[test]
fn loaded_experiment_rejects_filename_without_attachment_entry() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let run_parameters = registry
        .store_json_layer_blob(
            MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string()),
            &json!({ "columns": {} }),
            HashMap::new(),
        )
        .unwrap();
    let config = json!({
        "status": EXPERIMENT_STATUS_FINISHED,
        "attachments": {
            "entries": {},
            "filenames": {
                "missing": "missing.txt",
            },
        },
        "runs": [],
        "run_parameters": 0,
    });
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
        ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:invalid-filename-table").unwrap();
    let artifact =
        LocalArtifact::from_parts(registry, image_name, sealed_artifact.digest().clone());

    let err = SealedExperiment::from_artifact(artifact)
        .expect_err("filename table must reference existing attachments only");
    let messages = err
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("Attachment filename table references missing attachment `missing`"));
}

#[test]
fn loaded_experiment_rejects_config_run_attachment_not_listed_in_layers() {
    let temp = crate::artifact::local_registry::TempLocalRegistry::new().unwrap();
    let registry = temp.registry();
    let _outside_attachment = registry
        .store_layer_blob(
            MediaType::Other("application/json".to_string()),
            br#""outside""#,
            HashMap::new(),
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
        requested_image_name: None,
        attachments: AttachmentTable::new(),
        runs: vec![ExperimentConfigRun {
            run_id: 0,
            status: RUN_STATUS_FINISHED.to_string(),
            attachments: AttachmentTable::from_entries([("outside", LayerRef(1))]).unwrap(),
            trace: None,
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
        .contains("Failed to resolve run 0 attachment `outside` LayerRef 1"));
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
        requested_image_name: None,
        attachments: AttachmentTable::new(),
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
        let config = experiment_config(&artifact);
        let layers = artifact.layers().unwrap();
        let run_params = layer_from_ref(&layers, config.run_parameters);
        assert_eq!(
            run_params.media_type(),
            &MediaType::Other(RUN_PARAMETERS_MEDIA_TYPE.to_string())
        );
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
        assert_eq!(sealed.status(), &ExperimentStatus::Finished);
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
/// state back to the experiment. Registry payloads written before the
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
        let config = experiment_config(&artifact);
        assert!(config.runs.is_empty());
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
    assert_eq!(loaded.experiment_status(), Some(ExperimentStatus::Finished));
    let cells = loaded.run_parameter_cells().unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].run_id, 0);
    assert_eq!(cells[0].name, "solver");
    assert_eq!(cells[0].value, ParameterValue::String("scip".to_string()));
}

#[test]
fn experiment_dyn_run_rejects_second_trace() {
    let experiment = ExperimentDyn::with_temp_local_registry(Name::Anonymous).unwrap();
    let mut run = experiment.run().unwrap();
    run.store_trace(Trace::from_bytes(b"trace-1".to_vec()))
        .unwrap();

    let err = run
        .store_trace(Trace::from_bytes(b"trace-2".to_vec()))
        .expect_err("a RunDyn can store at most one trace");

    assert!(err.to_string().contains("already has a trace"));
    run.abandon();
    experiment.commit().unwrap();
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
fn experiment_dyn_publishes_failed_checkpoint() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:will-fail").unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), image_name.clone()).unwrap();
    experiment.log_json("dataset", json!("miplib2017")).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish().unwrap();
    }

    experiment
        .commit_failed_checkpoint("ValueError: failed")
        .unwrap();

    assert_eq!(experiment.state_name(), "failed");
    assert_eq!(experiment.image_name().unwrap(), image_name);
    assert!(registry_handle
        .registry()
        .resolve_image_name(&image_name)
        .unwrap()
        .is_none());
    let checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&image_name)
        .unwrap();
    let checkpoint = LocalArtifactDyn::open_in_registry_handle(
        registry_handle.clone(),
        checkpoint_image_name.clone(),
    )
    .unwrap();
    assert_eq!(checkpoint.image_name(), &checkpoint_image_name);

    assert!(checkpoint.annotations().unwrap().is_empty());

    let checkpoint_artifact = checkpoint.as_local_artifact();
    let config = experiment_config(&checkpoint_artifact);
    assert_eq!(config.status, EXPERIMENT_STATUS_FAILED);
    assert_eq!(config.requested_image_name, Some(image_name.to_string()));
    let err = SealedExperiment::from_artifact(checkpoint_artifact)
        .expect_err("failed checkpoint must not load as finished experiments");
    assert!(err.to_string().contains("status is failed"));
}

#[test]
fn experiment_dyn_recovers_failed_artifact_with_requested_image_name() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:failed-run").unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), image_name.clone()).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish_failed().unwrap();
    }
    experiment
        .commit_failed_checkpoint("RuntimeError: solve failed")
        .unwrap();
    let checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&image_name)
        .unwrap();
    let checkpoint = LocalArtifactDyn::open_in_registry_handle(
        registry_handle.clone(),
        checkpoint_image_name.clone(),
    )
    .unwrap();
    assert_eq!(checkpoint.image_name(), &checkpoint_image_name);

    let recovered = ExperimentDyn::restore_from_checkpoint_in_registry_handle(
        registry_handle,
        image_name.clone(),
    )
    .unwrap();
    assert!(recovered.is_unsealed());
    assert_eq!(recovered.image_name().unwrap(), image_name);
    {
        let mut run = recovered.run().unwrap();
        assert_eq!(run.run_id().unwrap(), 1);
        run.log_parameter("solver", "highs").unwrap();
        run.finish().unwrap();
    }
    let artifact = recovered.commit().unwrap();
    assert_eq!(artifact.image_name(), &image_name);

    let child_runs = recovered.runs().unwrap();
    assert_eq!(child_runs.len(), 2);
    assert_eq!(child_runs[0].status().as_str(), RUN_STATUS_FAILED);
    assert_eq!(child_runs[1].status().as_str(), RUN_STATUS_FINISHED);
}

#[test]
fn experiment_dyn_autosaves_on_run_close_and_recovers_with_requested_image_name() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:notebook").unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), image_name.clone()).unwrap();
    let checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&image_name)
        .unwrap();
    assert!(registry_handle
        .registry()
        .resolve_image_name(&checkpoint_image_name)
        .unwrap()
        .is_none());

    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish().unwrap();
    }

    let autosave = LocalArtifactDyn::open_in_registry_handle(
        registry_handle.clone(),
        checkpoint_image_name.clone(),
    )
    .expect("Run close should publish an autosave checkpoint");
    assert_eq!(autosave.image_name(), &checkpoint_image_name);
    assert!(autosave.annotations().unwrap().is_empty());

    let config = experiment_config(&autosave.as_local_artifact());
    assert_eq!(config.status, EXPERIMENT_STATUS_DRAFT);
    assert_eq!(config.requested_image_name, Some(image_name.to_string()));
    assert_eq!(config.runs.len(), 1);
    assert_eq!(config.runs[0].status, RUN_STATUS_FINISHED);
    assert_eq!(experiment.runs().unwrap().len(), 1);
    assert_eq!(experiment.run_parameter_cells().unwrap().len(), 1);
    let err = SealedExperiment::from_artifact(autosave.as_local_artifact())
        .expect_err("autosave checkpoint must not load as a finished experiment");
    assert!(err.to_string().contains("status is draft"));

    let recovered = ExperimentDyn::restore_from_checkpoint_in_registry_handle(
        registry_handle,
        image_name.clone(),
    )
    .unwrap();
    assert!(recovered.is_unsealed());
    assert_eq!(recovered.image_name().unwrap(), image_name);
    {
        let mut run = recovered.run().unwrap();
        assert_eq!(run.run_id().unwrap(), 1);
        run.log_parameter("solver", "highs").unwrap();
        run.finish().unwrap();
    }

    let artifact = recovered.commit().unwrap();
    assert_eq!(artifact.image_name(), &image_name);
    let child_runs = recovered.runs().unwrap();
    assert_eq!(child_runs.len(), 2);
    assert_eq!(child_runs[0].status().as_str(), RUN_STATUS_FINISHED);
    assert_eq!(child_runs[1].status().as_str(), RUN_STATUS_FINISHED);
}

#[test]
fn experiment_dyn_marks_keyboard_interrupt_checkpoint_separately() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/experiment-test:interrupt").unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), image_name.clone()).unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish_interrupted().unwrap();
    }

    let draft_checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&image_name)
        .unwrap();
    let autosave = LocalArtifactDyn::open_in_registry_handle(
        registry_handle.clone(),
        draft_checkpoint_image_name,
    )
    .unwrap();
    let autosave_config = experiment_config(&autosave.as_local_artifact());
    assert_eq!(autosave_config.status, EXPERIMENT_STATUS_DRAFT);
    assert_eq!(autosave_config.runs[0].status, RUN_STATUS_INTERRUPTED);

    experiment
        .commit_interrupted_checkpoint("KeyboardInterrupt")
        .unwrap();
    let checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&image_name)
        .unwrap();
    let checkpoint =
        LocalArtifactDyn::open_in_registry_handle(registry_handle.clone(), checkpoint_image_name)
            .unwrap();
    assert!(checkpoint.annotations().unwrap().is_empty());
    let config = experiment_config(&checkpoint.as_local_artifact());
    assert_eq!(config.status, EXPERIMENT_STATUS_INTERRUPTED);
    assert_eq!(config.requested_image_name, Some(image_name.to_string()));
    assert_eq!(config.runs[0].status, RUN_STATUS_INTERRUPTED);
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
fn experiment_dyn_rename_moves_autosave_checkpoint_ref() {
    let registry_handle = LocalRegistryHandle::temp().unwrap();
    let old_image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/rename-autosave:old").unwrap();
    let new_image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/rename-autosave:new").unwrap();
    let experiment =
        ExperimentDyn::with_registry_handle(registry_handle.clone(), old_image_name.clone())
            .unwrap();
    {
        let mut run = experiment.run().unwrap();
        run.log_parameter("solver", "scip").unwrap();
        run.finish().unwrap();
    }

    let old_checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&old_image_name)
        .unwrap();
    assert!(registry_handle
        .registry()
        .resolve_image_name(&old_checkpoint_image_name)
        .unwrap()
        .is_some());

    experiment.rename(new_image_name.clone()).unwrap();

    let new_checkpoint_image_name = registry_handle
        .registry()
        .experiment_checkpoint_image_name(&new_image_name)
        .unwrap();
    assert!(registry_handle
        .registry()
        .resolve_image_name(&old_checkpoint_image_name)
        .unwrap()
        .is_none());
    assert!(registry_handle
        .registry()
        .resolve_image_name(&new_checkpoint_image_name)
        .unwrap()
        .is_some());

    let recovered = ExperimentDyn::restore_from_checkpoint_in_registry_handle(
        registry_handle,
        new_image_name.clone(),
    )
    .unwrap();
    assert_eq!(recovered.image_name().unwrap(), new_image_name);
    assert_eq!(recovered.run_parameter_cells().unwrap().len(), 1);
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
    assert_eq!(loaded.attachment_names().unwrap().len(), 1);
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
