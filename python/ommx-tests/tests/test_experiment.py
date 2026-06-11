import math
import uuid
from dataclasses import dataclass
from typing import Any, ClassVar, cast

import pandas as pd
import pytest
from opentelemetry import trace as otel_trace

from ommx.adapter import SolverAdapter
from ommx.experiment import Experiment
from ommx.tracing import TraceResult, render_text_tree
from ommx.v1 import Instance, Solution

from conftest import get_test_exporter


@dataclass(frozen=True, slots=True)
class DummyDiagnostic:
    status: str
    bound: float


@dataclass(frozen=True, slots=True)
class DummyProgressDiagnostic:
    event: str
    node_count: int


@dataclass(frozen=True, slots=True)
class DummyMapKeyDiagnostic:
    values: dict[int, str]


@dataclass(frozen=True, slots=True)
class UnserializableDiagnostic:
    value: object


def _df_snap(df: pd.DataFrame) -> str:
    return df.to_string(na_rep="<NA>")


def _require_trace(trace: TraceResult | None) -> TraceResult:
    assert trace is not None
    return trace


def _trace_span_names(trace: TraceResult) -> set[str]:
    return {span.name for span in trace.spans}


def _trace_span_count(trace: TraceResult, name: str) -> int:
    return sum(span.name == name for span in trace.spans)


def _image_name(label: str) -> str:
    return f"example.com/ommx-tests/{label}:{uuid.uuid4().hex}"


def _single_trace_span(trace: TraceResult, name: str):
    spans = [span for span in trace.spans if span.name == name]
    assert len(spans) == 1
    return spans[0]


def _span_string_attributes(span) -> dict[str, str]:
    return {
        attribute.key: attribute.value.string_value
        for attribute in span.attributes
        if attribute.value.WhichOneof("value") == "string_value"
    }


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
    assert set(loaded.attachment_names) == {"dataset"}
    assert loaded.attachment_names == ["dataset"]
    assert loaded.get_json("dataset") == {"name": "miplib2017"}
    assert loaded.get_attachment("dataset") == {"name": "miplib2017"}
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0, 1, 2}
    assert set(runs[0].attachment_names) == {"candidate"}
    assert runs[0].attachment_names == ["candidate"]
    assert runs[0].get_json("candidate") == {"formulation": "a"}
    assert runs[0].get_attachment("candidate") == {"formulation": "a"}
    assert runs[1].attachment_names == []
    assert runs[2].attachment_names == []
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
    assert set(loaded.attachment_names) == {
        "dataset",
        "raw-config",
    }
    assert loaded.get_attachment("raw-config") == b"abc"
    assert loaded.get_blob("raw-config") == b"abc"
    with pytest.raises(RuntimeError, match="Expected media type"):
        loaded.get_json("raw-config")
    runs = {run.run_id: run for run in loaded.runs}
    assert set(runs) == {0}
    assert set(runs[0].attachment_names) == {"candidate", "solver-log"}
    assert runs[0].get_attachment("solver-log") == b"solved"
    assert runs[0].get_blob("solver-log") == b"solved"
    with pytest.raises(RuntimeError, match="Expected media type"):
        runs[0].get_instance("candidate")
    df = loaded.run_parameters_df()

    assert _df_snap(df) == snapshot


def test_typed_attachment_user_annotations_round_trip():
    instance = Instance.empty()
    instance.add_user_annotation("source", "experiment")
    solution = instance.evaluate({})
    solution.add_user_annotation("source", "run")

    with Experiment.with_temp_local_registry() as experiment:
        experiment.log_instance("instance", instance)
        with experiment.run() as run:
            run.log_solution("solution", solution)

    loaded = Experiment.from_artifact(experiment.artifact)

    loaded_instance = loaded.get_instance("instance")
    assert loaded_instance.get_user_annotation("source") == "experiment"

    loaded_solution = loaded.runs[0].get_solution("solution")
    assert loaded_solution.get_user_annotation("source") == "run"


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


def test_experiment_context_restores_failed_checkpoint_on_exception():
    image_name = _image_name("exception-checkpoint")
    experiments: list[Experiment] = []
    with pytest.raises(ValueError):
        with Experiment(image_name) as experiment:
            experiments.append(experiment)
            with experiment.run() as run:
                run.log_parameter("solver", "scip")
            raise ValueError("failed")

    experiment = experiments[0]
    with pytest.raises(RuntimeError, match="commit has failed"):
        experiment.artifact

    resumed = Experiment.restore_from_checkpoint(image_name)
    assert resumed.status is None
    assert resumed.image_name == image_name
    with resumed:
        with resumed.run() as run:
            assert run.run_id == 1
            run.log_parameter("solver", "highs")
    assert [run.status for run in resumed.runs] == ["finished", "finished"]
    assert list(resumed.run_parameters_df().index) == [0, 1]


def test_checkpoint_keeps_failed_run_and_can_be_restored():
    image_name = _image_name("failed-run-checkpoint")
    with pytest.raises(RuntimeError, match="solve failed"):
        with Experiment(image_name) as experiment:
            with experiment.run() as run:
                run.log_parameter("solver", "scip")
                run.log_json("before-failure", {"step": 1})
                raise RuntimeError("solve failed")

    resumed = Experiment.restore_from_checkpoint(image_name)
    assert resumed.status is None
    assert resumed.image_name == image_name
    assert resumed.runs[0].get_json("before-failure") == {"step": 1}
    assert resumed.run_parameters_df().loc[0, "solver"] == "scip"
    with resumed:
        with resumed.run() as run:
            assert run.run_id == 1
            run.log_parameter("solver", "highs")

    assert resumed.status == "finished"
    assert [run.status for run in resumed.runs] == ["failed", "finished"]
    assert resumed.runs[0].get_json("before-failure") == {"step": 1}
    assert resumed.run_parameters_df().loc[0, "solver"] == "scip"
    assert list(resumed.run_parameters_df().index) == [0, 1]


def test_run_close_updates_live_state():
    experiment = Experiment.with_temp_local_registry()

    with experiment.run() as run:
        run.log_parameter("solver", "scip")
        run.log_json("before-commit", {"step": 1})

    assert [run.status for run in experiment.runs] == ["finished"]
    assert experiment.runs[0].get_json("before-commit") == {"step": 1}
    assert experiment.run_parameters_df().loc[0, "solver"] == "scip"


def test_keyboard_interrupt_records_interrupted_run_and_checkpoint():
    image_name = _image_name("keyboard-interrupt-checkpoint")
    with pytest.raises(KeyboardInterrupt):
        with Experiment(image_name) as experiment:
            with experiment.run() as run:
                run.log_parameter("solver", "scip")
                raise KeyboardInterrupt

    resumed = Experiment.restore_from_checkpoint(image_name)
    with resumed:
        pass
    assert [run.status for run in resumed.runs] == ["interrupted"]
    assert resumed.run_parameters_df().loc[0, "solver"] == "scip"


def test_store_trace_records_run_scope_in_artifact():
    experiment = Experiment.with_temp_local_registry(store_trace=True)
    experiment.log_json("dataset", {"name": "miplib2017"})
    with experiment.run() as run:
        run.log_parameter("solver", "highs")
    artifact = experiment.commit()

    loaded = Experiment.from_artifact(artifact)
    assert set(loaded.attachment_names) == {"dataset"}

    trace = _require_trace(loaded.runs[0].trace)
    names = _trace_span_names(trace)
    assert "Run" in names
    tree = render_text_tree(trace)
    assert "Run" in tree
    assert "{scope=ommx.experiment}" in tree


def test_store_trace_records_log_solve_scope_in_artifact():
    class DummyAdapter(SolverAdapter):
        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
        ) -> Solution:
            assert diagnostics is None
            tracer = otel_trace.get_tracer("dummy_adapter")
            with tracer.start_as_current_span("solve") as span:
                span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
                return ommx_instance.evaluate({})

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()

    with Experiment.with_temp_local_registry(store_trace=True) as experiment:
        with experiment.run() as run:
            run.log_solve(DummyAdapter, instance)

    trace = _require_trace(experiment.runs[0].trace)
    run_span = _single_trace_span(trace, "Run")
    solve_span = _single_trace_span(trace, "solve")
    assert solve_span.parent_span_id == run_span.span_id
    assert _span_string_attributes(solve_span)["adapter"].endswith("DummyAdapter")

    tree = render_text_tree(trace)
    assert "solve" in tree
    assert "{scope=dummy_adapter}" in tree
    assert "adapter='" in tree


def test_fork_store_trace_carries_parent_run_trace():
    with Experiment.with_temp_local_registry(store_trace=True) as parent:
        with parent.run() as run:
            run.log_parameter("solver", "base")

    parent_trace = _require_trace(parent.runs[0].trace)

    with parent.fork(
        "ghcr.io/jij-inc/ommx/forked-trace-experiment:latest",
        store_trace=True,
    ) as child:
        with child.run() as run:
            assert run.run_id == 1
            run.log_parameter("solver", "child")

    traces = [_require_trace(run.trace) for run in child.runs]
    assert traces[0].otlp_protobuf() == parent_trace.otlp_protobuf()
    assert sum(_trace_span_count(trace, "Run") for trace in traces) == 2


def test_fork_store_trace_can_add_trace_to_new_run_only():
    with Experiment.with_temp_local_registry() as parent:
        with parent.run() as run:
            run.log_parameter("solver", "base")

    assert parent.runs[0].trace is None

    child = parent.fork(
        "ghcr.io/jij-inc/ommx/forked-trace-new-run:latest",
        store_trace=True,
    )
    with child.run() as run:
        assert run.run_id == 1
        run.log_parameter("solver", "child")
    child.commit()

    traces = [run.trace for run in child.runs]
    assert traces[0] is None
    trace = traces[1]
    assert trace is not None
    assert _trace_span_count(trace, "Run") == 1


def test_default_experiment_context_does_not_store_trace():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_parameter("solver", "highs")

    assert Experiment.from_artifact(experiment.artifact).runs[0].trace is None


def test_experiment_context_does_not_emit_experiment_span():
    exporter = get_test_exporter()
    exporter.clear()

    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_parameter("solver", "highs")

    assert "ommx.experiment" not in {span.name for span in exporter.spans}


def test_run_context_enter_failure_closes_partial_span(monkeypatch):
    from opentelemetry import trace as otel_trace

    experiment = Experiment.with_temp_local_registry()
    run = experiment.run()

    class BrokenSpan:
        def set_attribute(self, name, value):
            raise RuntimeError("run attribute unavailable")

    real_get_current_span = otel_trace.get_current_span

    def get_current_span(*args, **kwargs):
        if args or kwargs:
            return real_get_current_span(*args, **kwargs)
        return BrokenSpan()

    with monkeypatch.context() as patch:
        patch.setattr(otel_trace, "get_current_span", get_current_span)
        with pytest.raises(RuntimeError, match="run attribute unavailable"):
            run.__enter__()

    assert not otel_trace.get_current_span().get_span_context().is_valid

    with run:
        run.log_parameter("solver", "highs")
    experiment.commit()


def test_run_context_exit_failure_keeps_entered_state(monkeypatch):
    from opentelemetry import trace as otel_trace

    experiment = Experiment.with_temp_local_registry()
    run = experiment.run()

    class BrokenContextManager:
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc_value, traceback):
            raise RuntimeError("run span close failed")

    class FakeTracer:
        def start_as_current_span(self, name):
            return BrokenContextManager()

    with monkeypatch.context() as patch:
        patch.setattr(otel_trace, "get_tracer", lambda *args, **kwargs: FakeTracer())
        run.__enter__()
        with pytest.raises(RuntimeError, match="run span close failed"):
            run.__exit__()
        with pytest.raises(RuntimeError, match="already been entered"):
            run.__enter__()
        with pytest.raises(RuntimeError, match="Run handle"):
            experiment.commit()


def test_store_trace_requires_run_context_manager_before_run_mutation():
    experiment = Experiment.with_temp_local_registry(store_trace=True)
    experiment.log_json("dataset", {"name": "miplib2017"})
    run = experiment.run()

    with pytest.raises(RuntimeError, match="store_trace=True requires using Run"):
        run.log_parameter("solver", "highs")

    with pytest.raises(RuntimeError, match="store_trace=True requires using Run"):
        run.finish()


def test_run_finish_rejects_active_context_manager():
    experiment = Experiment.with_temp_local_registry()

    with pytest.raises(RuntimeError, match="Run context is active"):
        with experiment.run() as run:
            run.finish()

    artifact = experiment.commit()
    runs = Experiment.from_artifact(artifact).runs
    assert len(runs) == 1
    assert runs[0].status == "failed"


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
        Experiment.from_artifact(experiment.artifact)
        .run_parameters_df()
        .loc[0, "solver"]
        == "highs"
    )
    assert (
        Experiment.from_artifact(old_artifact).run_parameters_df().loc[0, "solver"]
        == "highs"
    )


def test_fork_committed_experiment_creates_unsealed_child():
    with Experiment.with_temp_local_registry() as parent:
        parent.log_json("dataset", {"name": "miplib2017"})
        with parent.run() as run:
            run.log_parameter("solver", "base")

    with parent.fork("ghcr.io/jij-inc/ommx/forked-experiment:latest") as child:
        assert child.image_name == "ghcr.io/jij-inc/ommx/forked-experiment:latest"
        assert "state='unsealed'" in repr(child)
        with child.run() as run:
            assert run.run_id == 1
            run.log_parameter("solver", "child")

    parent_df = parent.run_parameters_df()
    assert list(parent_df.index) == [0]
    assert parent_df.loc[0, "solver"] == "base"

    child_df = child.run_parameters_df()
    assert list(child_df.index) == [0, 1]
    assert child_df.loc[0, "solver"] == "base"
    assert child_df.loc[1, "solver"] == "child"
    assert child.get_json("dataset") == {"name": "miplib2017"}
    assert [run.run_id for run in parent.runs] == [0]
    assert [run.run_id for run in child.runs] == [0, 1]


def test_fork_rejects_uncommitted_experiment():
    experiment = Experiment.with_temp_local_registry()

    with pytest.raises(RuntimeError, match="must be committed"):
        experiment.fork()


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


def test_run_context_records_failed_run_on_exception():
    experiment = Experiment.with_temp_local_registry()
    with pytest.raises(ValueError):
        with experiment.run() as run:
            run.log_parameter("solver", "scip")
            raise ValueError("failed")

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    df = loaded.run_parameters_df()

    assert len(loaded.runs) == 1
    assert loaded.runs[0].status == "failed"
    assert df.loc[0, "solver"] == "scip"


def test_log_parameter_rejects_python_int_outside_i64():
    experiment = Experiment.with_temp_local_registry()
    with experiment.run() as run:
        with pytest.raises(OverflowError, match="int64"):
            run.log_parameter("too_large", 2**63)


def test_log_solve_logs_input_solution_and_adapter_options():
    class DummyAdapter(SolverAdapter):
        seen_kwargs: ClassVar[list[dict[str, object]]] = []

        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            assert diagnostics is None
            cls.seen_kwargs.append(kwargs)
            solution = ommx_instance.evaluate({})
            solution.add_user_annotation("adapter", "dummy")
            return solution

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    instance.add_user_annotation("source", "solve-input")
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
    assert runs[0].attachment_names == []
    assert [solve.solve_id for solve in runs[0].solves] == [0, 1]

    first_solve = runs[0].solves[0]
    assert first_solve.status == "finished"
    assert isinstance(first_solve.input, Instance)
    assert first_solve.input.get_user_annotation("source") == "solve-input"
    first_output = first_solve.output
    assert isinstance(first_output, Solution)
    assert first_output.get_user_annotation("adapter") == "dummy"
    assert first_output.feasible
    assert str(first_solve.adapter).endswith("DummyAdapter")
    assert isinstance(first_solve.adapter_options, dict)
    assert first_solve.adapter_options == {
        "time_limit": 1.5,
        "verbose": True,
        "label": "baseline",
    }
    assert first_solve.diagnostics == []

    second_solve = runs[0].solves[1]
    assert second_solve.status == "finished"
    assert isinstance(second_solve.adapter_options, dict)
    assert second_solve.adapter_options == {
        "time_limit": 2.0,
        "label": "pricing",
    }
    assert second_solve.diagnostics == []

    # Adapter options are solve-scoped metadata, not Run parameters.
    df = loaded.run_parameters_df()
    assert df.shape == (1, 0)


def test_log_solve_records_adapter_diagnostics():
    class DiagnosticAdapter(SolverAdapter):
        seen_kwargs: ClassVar[list[dict[str, object]]] = []

        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            cls.seen_kwargs.append(kwargs)
            assert diagnostics is not None
            diagnostics.record(DummyProgressDiagnostic(event="node", node_count=10))
            diagnostics.record(DummyDiagnostic(status="terminated", bound=math.inf))
            diagnostics.record(DummyMapKeyDiagnostic(values={1: "root"}))
            return ommx_instance.evaluate({})

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    experiment = Experiment.with_temp_local_registry()
    DiagnosticAdapter.seen_kwargs = []

    with experiment.run() as run:
        solution = run.log_solve(
            DiagnosticAdapter,
            instance,
            store_diagnostics=True,
            time_limit=1.5,
        )
        assert solution.feasible

    assert DiagnosticAdapter.seen_kwargs == [{"time_limit": 1.5}]

    artifact = experiment.commit()
    loaded = Experiment.from_artifact(artifact)
    solve = loaded.runs[0].solves[0]

    assert solve.adapter_options == {"time_limit": 1.5}
    assert solve.diagnostics == [
        {"event": "node", "node_count": 10},
        {"status": "terminated", "bound": math.inf},
        {"values": {1: "root"}},
    ]
    assert math.isinf(solve.diagnostics[1]["bound"])


def test_log_solve_records_solve_without_unserializable_diagnostics():
    class UnserializableDiagnosticAdapter(SolverAdapter):
        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            assert diagnostics is not None
            diagnostics.record(UnserializableDiagnostic(value=object()))
            solution = ommx_instance.evaluate({})
            solution.add_user_annotation("adapter", "diagnostics-fallback")
            return solution

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    experiment = Experiment.with_temp_local_registry()

    with experiment.run() as run:
        solution = run.log_solve(
            UnserializableDiagnosticAdapter,
            instance,
            store_diagnostics=True,
            label="unserializable-diagnostics",
        )
        assert solution.feasible

    loaded = Experiment.from_artifact(experiment.commit())
    run = loaded.runs[0]
    assert run.status == "finished"
    assert [solve.solve_id for solve in run.solves] == [0]

    solve = run.solves[0]
    assert solve.status == "finished"
    assert solve.output is not None
    assert solve.output.get_user_annotation("adapter") == "diagnostics-fallback"
    assert solve.adapter_options == {"label": "unserializable-diagnostics"}
    assert solve.diagnostics == []


def test_failed_run_preserves_completed_solves_after_adapter_exception():
    class FailingThirdAdapter(SolverAdapter):
        calls: ClassVar[int] = 0

        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            assert diagnostics is not None
            cls.calls += 1
            if cls.calls == 3:
                diagnostics.record(
                    DummyDiagnostic(status="about-to-fail", bound=math.inf)
                )
                raise RuntimeError("backend crashed")
            return ommx_instance.evaluate({})

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    instance.add_user_annotation("source", "failed-solve-input")
    experiment = Experiment.with_temp_local_registry()
    FailingThirdAdapter.calls = 0

    with pytest.raises(RuntimeError, match="backend crashed"):
        with experiment.run() as run:
            run.log_solve(
                FailingThirdAdapter,
                instance,
                store_diagnostics=True,
                label="first",
            )
            run.log_solve(
                FailingThirdAdapter,
                instance,
                store_diagnostics=True,
                label="second",
            )
            run.log_solve(
                FailingThirdAdapter,
                instance,
                store_diagnostics=True,
                label="third",
            )

    loaded = Experiment.from_artifact(experiment.commit())
    run = loaded.runs[0]
    assert run.status == "failed"
    assert [solve.solve_id for solve in run.solves] == [0, 1, 2]
    assert [solve.status for solve in run.solves] == [
        "finished",
        "finished",
        "failed",
    ]
    assert [solve.adapter_options["label"] for solve in run.solves] == [
        "first",
        "second",
        "third",
    ]
    assert all(
        solve.output is not None and solve.output.feasible for solve in run.solves[:2]
    )
    failed_solve = run.solves[2]
    assert isinstance(failed_solve.input, Instance)
    assert failed_solve.input.get_user_annotation("source") == "failed-solve-input"
    assert failed_solve.output is None
    assert failed_solve.diagnostics == [{"status": "about-to-fail", "bound": math.inf}]


def test_log_solve_preserves_keyboard_interrupt_type_and_records_interrupted_solve():
    class InterruptingAdapter(SolverAdapter):
        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            assert diagnostics is not None
            diagnostics.record(
                DummyDiagnostic(status="before-interrupt", bound=math.inf)
            )
            raise KeyboardInterrupt

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    experiment = Experiment.with_temp_local_registry()

    with pytest.raises(KeyboardInterrupt):
        with experiment.run() as run:
            run.log_solve(
                InterruptingAdapter,
                instance,
                store_diagnostics=True,
                label="interrupt",
            )

    loaded = Experiment.from_artifact(experiment.commit())
    run = loaded.runs[0]
    assert run.status == "interrupted"
    assert [solve.solve_id for solve in run.solves] == [0]

    solve = run.solves[0]
    assert solve.status == "interrupted"
    assert solve.output is None
    assert solve.adapter_options == {"label": "interrupt"}
    assert solve.diagnostics == [{"status": "before-interrupt", "bound": math.inf}]


def test_log_solve_rejects_non_solver_adapter():
    class DummyAdapter:
        @classmethod
        def solve(cls, ommx_instance: Instance) -> Solution:
            return ommx_instance.evaluate({})

    instance = Instance.empty()
    experiment = Experiment.with_temp_local_registry()

    with experiment.run() as run:
        with pytest.raises(TypeError, match="ommx.adapter.SolverAdapter"):
            run.log_solve(cast(Any, DummyAdapter), instance)


def test_log_solve_rejects_non_json_kwargs_before_solving():
    class DummyAdapter(SolverAdapter):
        called: ClassVar[bool] = False

        @classmethod
        def solve(
            cls,
            ommx_instance: Instance,
            *,
            diagnostics: Any | None = None,
            **kwargs: object,
        ) -> Solution:
            assert diagnostics is not None
            cls.called = True
            return ommx_instance.evaluate({})

        @property
        def solver_input(self) -> Any:
            raise NotImplementedError

        def decode(self, data: Any) -> Solution:
            raise NotImplementedError

    instance = Instance.empty()
    experiment = Experiment.with_temp_local_registry()
    DummyAdapter.called = False

    with experiment.run() as run:
        with pytest.raises(RuntimeError, match="JSON-serializable"):
            run.log_solve(DummyAdapter, instance, callback=object())

    assert not DummyAdapter.called
