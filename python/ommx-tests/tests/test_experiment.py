import pandas as pd
import pytest

from ommx.experiment import Experiment


def test_view_run_parameters_from_committed_artifact():
    def scenario(experiment: Experiment) -> pd.DataFrame:
        experiment.log_json("dataset", {"name": "miplib2017"})

        run0 = experiment.run()
        run0.log_parameter("solver", "scip")
        run0.log_parameter("time_limit", 20.0)
        run0.log_json("candidate", {"formulation": "a"})
        run0.finish()

        run1 = experiment.run()
        run1.log_parameter("solver", "highs")
        run1.log_parameter("presolve", True)
        run1.finish()

        artifact = experiment.commit()
        loaded = Experiment.from_artifact(artifact)
        assert {
            (record.space, record.run_id, record.name) for record in loaded.records
        } == {
            ("experiment", None, "dataset"),
            ("run", 0, "candidate"),
        }
        return loaded.run_parameters_df()

    df = Experiment.with_temp_local_registry(scenario)
    assert list(df.index) == [0, 1]
    assert df.index.name == "run_id"
    assert df.loc[0, "solver"] == "scip"
    assert df.loc[1, "solver"] == "highs"
    assert bool(df.loc[1, "presolve"]) is True
    assert df.loc[0, "time_limit"] == 20.0
    assert pd.isna(df.loc[1, "time_limit"])


def test_create_experiment_run_records_and_commit():
    def scenario(experiment: Experiment) -> pd.DataFrame:
        assert ".ommx.local/experiment:" in experiment.image_name

        experiment.log_json("dataset", {"name": "miplib2017"})
        experiment.log_record("raw-config", "application/octet-stream", b"abc")

        run = experiment.run()
        assert run.run_id == 0
        run.log_parameter("solver", "scip")
        run.log_parameter("time_limit", 20.0)
        run.log_json("candidate", {"formulation": "a"})
        run.log_record("solver-log", "text/plain", b"solved")
        run.finish()

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
        return loaded.run_parameters_df()

    df = Experiment.with_temp_local_registry(scenario)
    assert list(df.index) == [0]
    assert df.loc[0, "solver"] == "scip"
    assert df.loc[0, "time_limit"] == 20.0


def test_commit_rejects_open_run():
    def scenario(experiment: Experiment) -> None:
        run = experiment.run()

        with pytest.raises(RuntimeError, match="Run handle"):
            experiment.commit()

        run.finish()
        experiment.commit()

    Experiment.with_temp_local_registry(scenario)


def test_temp_registry_requires_runs_to_finish_before_callback_returns():
    escaped = {}

    def scenario(experiment: Experiment) -> None:
        escaped["run"] = experiment.run()

    with pytest.raises(RuntimeError, match="must be finished"):
        Experiment.with_temp_local_registry(scenario)
    with pytest.raises(RuntimeError, match="Parent Experiment"):
        escaped["run"].run_id
