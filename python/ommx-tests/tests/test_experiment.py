import pandas as pd
import pytest

from ommx.experiment import Experiment


def test_view_run_parameters_from_committed_artifact():
    df: pd.DataFrame | None = None
    with Experiment.on_temp_local_registry() as experiment:
        experiment.log_json("dataset", {"name": "miplib2017"})

        with experiment.run() as run0:
            run0.log_parameter("solver", "scip")
            run0.log_parameter("time_limit", 20.0)
            run0.log_json("candidate", {"formulation": "a"})

        with experiment.run() as run1:
            run1.log_parameter("solver", "highs")
            run1.log_parameter("presolve", True)

    artifact = experiment.artifact
    loaded = Experiment.from_artifact(artifact)
    assert {
        (record.space, record.run_id, record.name) for record in loaded.records
    } == {
        ("experiment", None, "dataset"),
        ("run", 0, "candidate"),
    }
    df = loaded.run_parameters_df()

    assert df is not None
    assert list(df.index) == [0, 1]
    assert df.index.name == "run_id"
    assert df.loc[0, "solver"] == "scip"
    assert df.loc[1, "solver"] == "highs"
    assert bool(df.loc[1, "presolve"]) is True
    assert df.loc[0, "time_limit"] == 20.0
    assert pd.isna(df.loc[1, "time_limit"])


def test_create_experiment_run_records_and_commit():
    df: pd.DataFrame | None = None
    experiment = Experiment.on_temp_local_registry()
    assert ".ommx.local/experiment:" in experiment.image_name

    experiment.log_json("dataset", {"name": "miplib2017"})
    experiment.log_record("raw-config", "application/octet-stream", b"abc")

    with experiment.run() as run:
        assert run.run_id == 0
        run.log_parameter("solver", "scip")
        run.log_parameter("time_limit", 20.0)
        run.log_json("candidate", {"formulation": "a"})
        run.log_record("solver-log", "text/plain", b"solved")

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    assert {
        (record.space, record.run_id, record.name) for record in loaded.records
    } == {
        ("experiment", None, "dataset"),
        ("experiment", None, "raw-config"),
        ("run", 0, "candidate"),
        ("run", 0, "solver-log"),
    }
    df = loaded.run_parameters_df()

    assert df is not None
    assert list(df.index) == [0]
    assert df.loc[0, "solver"] == "scip"
    assert df.loc[0, "time_limit"] == 20.0


def test_commit_rejects_open_run():
    experiment = Experiment.on_temp_local_registry()
    with experiment.run():
        with pytest.raises(RuntimeError, match="Run handle"):
            experiment.commit()
    experiment.commit()


def test_temp_registry_lives_with_artifact_after_experiment_drop():
    experiment = Experiment.on_temp_local_registry()
    with experiment.run() as run:
        run.log_parameter("solver", "scip")

    artifact = experiment.commit()
    del experiment

    loaded = Experiment.from_artifact(artifact)
    del artifact

    df = loaded.run_parameters_df()
    assert list(df.index) == [0]
    assert df.loc[0, "solver"] == "scip"


def test_experiment_context_does_not_commit_on_exception():
    experiments: list[Experiment] = []
    with pytest.raises(ValueError):
        with Experiment.on_temp_local_registry() as experiment:
            experiments.append(experiment)
            with experiment.run() as run:
                run.log_parameter("solver", "scip")
            raise ValueError("failed")

    experiment = experiments[0]
    with pytest.raises(RuntimeError, match="must be committed"):
        experiment.artifact


def test_run_context_does_not_finish_on_exception():
    df: pd.DataFrame | None = None
    experiment = Experiment.on_temp_local_registry()
    with pytest.raises(ValueError):
        with experiment.run() as run:
            run.log_parameter("solver", "scip")
            raise ValueError("failed")

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    df = loaded.run_parameters_df()

    assert df is not None
    assert df.empty
