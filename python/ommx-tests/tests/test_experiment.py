from typing import Any, ClassVar, cast

import pandas as pd
import pytest

from ommx.adapter import SolverAdapter
from ommx.experiment import Experiment
from ommx.v1 import Instance, Solution

_RECORD_NAME = "org.ommx.record.name"


def _df_snap(df: pd.DataFrame) -> str:
    return df.to_string(na_rep="<NA>")


def _record_names(records) -> set[str]:
    return {record.annotations[_RECORD_NAME] for record in records}


def test_view_run_parameters_from_committed_artifact(snapshot):
    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_json("dataset", {"name": "miplib2017"})

        with experiment.run() as run0:
            run0.log_parameter("solver", "scip")
            run0.log_parameter("time_limit", 20.0)
            run0.log_json("candidate", {"formulation": "a"})

        with experiment.run() as run1:
            run1.log_parameter("solver", "highs")
            run1.log_parameter("presolve", True)

        with experiment.run():
            pass

    artifact = experiment.artifact
    loaded = Experiment.from_artifact(artifact)
    assert _record_names(loaded.experiment_records) == {"dataset"}
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0, 1, 2}
    assert _record_names(runs[0].records) == {"candidate"}
    assert runs[1].records == []
    assert runs[2].records == []
    df = loaded.run_parameters_df()

    assert _df_snap(df) == snapshot


def test_create_experiment_run_records_and_commit(snapshot):
    experiment = Experiment.with_temp_local_registry()
    assert ".ommx.local/experiment:" in experiment.image_name
    assert "state='unsealed'" in repr(experiment)
    assert "open_runs=0" in repr(experiment)

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
    assert _record_names(loaded.experiment_records) == {
        "dataset",
        "raw-config",
    }
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0}
    assert _record_names(runs[0].records) == {"candidate", "solver-log"}
    df = loaded.run_parameters_df()

    assert _df_snap(df) == snapshot


def test_commit_rejects_open_run():
    experiment = Experiment.with_temp_local_registry()
    with experiment.run():
        assert "open_runs=1" in repr(experiment)
        with pytest.raises(RuntimeError, match="Run handle"):
            experiment.commit()
    experiment.commit()
    assert "state='sealed'" in repr(experiment)


def test_temp_registry_lives_with_artifact_after_experiment_drop():
    experiment = Experiment.with_temp_local_registry()
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
        with Experiment.with_temp_local_registry() as experiment:
            experiments.append(experiment)
            with experiment.run() as run:
                run.log_parameter("solver", "scip")
            raise ValueError("failed")

    experiment = experiments[0]
    with pytest.raises(RuntimeError, match="must be committed"):
        experiment.artifact


def test_run_context_does_not_finish_on_exception():
    experiment = Experiment.with_temp_local_registry()
    with pytest.raises(ValueError):
        with experiment.run() as run:
            run.log_parameter("solver", "scip")
            raise ValueError("failed")

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    df = loaded.run_parameters_df()

    assert df is not None
    assert df.empty


def test_log_parameter_rejects_python_int_outside_i64():
    experiment = Experiment.with_temp_local_registry()
    with experiment.run() as run:
        with pytest.raises(OverflowError, match="int64"):
            run.log_parameter("too_large", 2**63)


def test_log_solve_records_input_solution_and_scalar_kwargs():
    class DummyAdapter(SolverAdapter):
        seen_kwargs: ClassVar[dict[str, object] | None] = None

        @classmethod
        def solve(cls, ommx_instance: Instance, **kwargs: object) -> Solution:
            cls.seen_kwargs = kwargs
            return ommx_instance.evaluate({})

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    experiment = Experiment.with_temp_local_registry()

    with experiment.run() as run:
        solution = run.log_solve(
            DummyAdapter,
            instance,
            time_limit=1.5,
            verbose=True,
            label="baseline",
        )
        assert solution.feasible

    assert DummyAdapter.seen_kwargs == {
        "time_limit": 1.5,
        "verbose": True,
        "label": "baseline",
    }

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    runs = {run.run_id: run for run in loaded.runs}
    assert _record_names(runs[0].records) == {"input", "solution"}

    df = loaded.run_parameters_df()
    assert str(df.loc[0, "adapter"]).endswith("DummyAdapter")
    assert df.loc[0, "time_limit"] == 1.5
    assert bool(df.loc[0, "verbose"]) is True
    assert df.loc[0, "label"] == "baseline"


def test_log_solve_rejects_non_solver_adapter():
    class DummyAdapter:
        @classmethod
        def solve(cls, ommx_instance: Instance) -> Solution:
            return ommx_instance.evaluate({})

    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    experiment = Experiment.with_temp_local_registry()

    with experiment.run() as run:
        with pytest.raises(TypeError, match="ommx.adapter.SolverAdapter"):
            run.log_solve(cast(Any, DummyAdapter), instance)
