from typing import Any, ClassVar, cast

import pandas as pd
import pytest

from ommx.adapter import SolverAdapter
from ommx.experiment import Experiment
from ommx.v1 import Instance, Solution

_ATTACHMENT_NAME = "org.ommx.attachment.name"


def _df_snap(df: pd.DataFrame) -> str:
    return df.to_string(na_rep="<NA>")


def _attachment_names(attachments) -> set[str]:
    return {attachment.annotations[_ATTACHMENT_NAME] for attachment in attachments}


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
    assert _attachment_names(loaded.experiment_attachments) == {"dataset"}
    assert loaded.attachment_names == ["dataset"]
    assert loaded.get_json("dataset") == {"name": "miplib2017"}
    assert loaded.get_attachment("dataset") == {"name": "miplib2017"}
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0, 1, 2}
    assert _attachment_names(runs[0].attachments) == {"candidate"}
    assert runs[0].attachment_names == ["candidate"]
    assert runs[0].get_json("candidate") == {"formulation": "a"}
    assert runs[0].get_attachment("candidate") == {"formulation": "a"}
    assert runs[1].attachments == []
    assert runs[2].attachments == []
    df = loaded.run_parameters_df()

    assert _df_snap(df) == snapshot


def test_create_experiment_run_attachments_and_commit(snapshot):
    experiment = Experiment.with_temp_local_registry()
    assert ".ommx.local/experiment:" in experiment.image_name
    assert "state='unsealed'" in repr(experiment)
    assert "open_runs=0" in repr(experiment)

    experiment.log_json("dataset", {"name": "miplib2017"})
    experiment.log_attachment("raw-config", "application/octet-stream", b"abc")

    with experiment.run() as run:
        assert run.run_id == 0
        run.log_parameter("solver", "scip")
        run.log_parameter("time_limit", 20.0)
        run.log_json("candidate", {"formulation": "a"})
        run.log_attachment("solver-log", "text/plain", b"solved")

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    assert _attachment_names(loaded.experiment_attachments) == {
        "dataset",
        "raw-config",
    }
    assert loaded.get_attachment("raw-config") == b"abc"
    assert loaded.get_blob("raw-config") == b"abc"
    with pytest.raises(RuntimeError, match="Expected media type"):
        loaded.get_json("raw-config")
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0}
    assert _attachment_names(runs[0].attachments) == {"candidate", "solver-log"}
    assert runs[0].get_attachment("solver-log") == b"solved"
    assert runs[0].get_blob("solver-log") == b"solved"
    with pytest.raises(RuntimeError, match="Expected media type"):
        runs[0].get_instance("candidate")
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


def test_rename_after_context_commit_updates_artifact_name():
    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_json("dataset", {"name": "miplib2017"})
        with experiment.run() as run:
            run.log_parameter("solver", "highs")

    old_artifact = experiment.artifact
    old_image_name = experiment.image_name
    new_image_name = "ghcr.io/jij-inc/ommx/renamed-experiment:latest"

    experiment.rename(new_image_name)

    assert old_artifact.image_name == old_image_name
    assert experiment.image_name == new_image_name
    assert experiment.artifact.image_name == new_image_name
    assert (
        Experiment.from_artifact(experiment.artifact).run_parameters_df().loc[0, "solver"]
        == "highs"
    )
    assert Experiment.from_artifact(old_artifact).run_parameters_df().loc[0, "solver"] == "highs"


def test_push_rejects_uncommitted_experiment():
    experiment = Experiment.with_temp_local_registry()

    with pytest.raises(RuntimeError, match="must be committed"):
        experiment.push()


def test_save_exports_committed_experiment_archive(tmp_path):
    archive_path = tmp_path / "experiment.ommx"
    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_json("dataset", {"name": "miplib2017"})
        with experiment.run() as run:
            run.log_parameter("solver", "highs")

    experiment.save(archive_path)

    assert archive_path.is_file()
    assert archive_path.stat().st_size > 0
    loaded = Experiment.import_archive(archive_path)
    assert loaded.run_parameters_df().loc[0, "solver"] == "highs"


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


def test_log_solve_logs_input_solution_and_adapter_options():
    class DummyAdapter(SolverAdapter):
        seen_kwargs: ClassVar[list[dict[str, object]]] = []

        @classmethod
        def solve(cls, ommx_instance: Instance, **kwargs: object) -> Solution:
            cls.seen_kwargs.append(kwargs)
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
    DummyAdapter.seen_kwargs = []

    with experiment.run() as run:
        solution = run.log_solve(
            DummyAdapter,
            instance,
            time_limit=1.5,
            verbose=True,
            label="baseline",
        )
        assert solution.feasible
        solution = run.log_solve(
            DummyAdapter,
            instance,
            time_limit=2.0,
            label="pricing",
        )
        assert solution.feasible

    assert DummyAdapter.seen_kwargs == [
        {
            "time_limit": 1.5,
            "verbose": True,
            "label": "baseline",
        },
        {
            "time_limit": 2.0,
            "label": "pricing",
        },
    ]

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    runs = {run.run_id: run for run in loaded.runs}
    assert runs[0].attachments == []
    assert [solve.solve_id for solve in runs[0].solves] == [0, 1]

    first_solve = runs[0].solves[0]
    assert isinstance(first_solve.input, Instance)
    assert isinstance(first_solve.output, Solution)
    assert first_solve.output.feasible
    assert str(first_solve.adapter).endswith("DummyAdapter")
    assert isinstance(first_solve.adapter_options, dict)
    assert first_solve.adapter_options == {
        "time_limit": 1.5,
        "verbose": True,
        "label": "baseline",
    }

    second_solve = runs[0].solves[1]
    assert isinstance(second_solve.adapter_options, dict)
    assert second_solve.adapter_options == {
        "time_limit": 2.0,
        "label": "pricing",
    }

    # Adapter options are solve-scoped metadata, not Run parameters.
    df = loaded.run_parameters_df()
    assert df.shape == (1, 0)


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


def test_log_solve_rejects_non_json_kwargs_before_solving():
    class DummyAdapter(SolverAdapter):
        called: ClassVar[bool] = False

        @classmethod
        def solve(cls, ommx_instance: Instance, **kwargs: object) -> Solution:
            cls.called = True
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
    DummyAdapter.called = False

    with experiment.run() as run:
        with pytest.raises(RuntimeError, match="JSON-serializable"):
            run.log_solve(DummyAdapter, instance, callback=object())

    assert not DummyAdapter.called
