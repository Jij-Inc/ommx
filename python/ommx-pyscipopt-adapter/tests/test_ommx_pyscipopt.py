import math
import warnings
from typing import Any, cast

import pandas as pd
import pytest

from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx.experiment import Experiment
from ommx.v1 import Instance, Constraint, DecisionVariable, Solution

from ommx_pyscipopt_adapter import (
    SCIPDiagnosticsAnalyzer,
    OMMXPySCIPOptAdapter,
    SCIPProgressSnapshot,
    SCIPTerminationReport,
)
from ommx_pyscipopt_adapter.exception import OMMXPySCIPOptAdapterError


def _knapsack_instance() -> Instance:
    p = [10, 13, 18, 32, 7, 15]
    w = [11, 15, 20, 35, 10, 33]
    x = [DecisionVariable.binary(i) for i in range(6)]
    return Instance.from_components(
        decision_variables=x,
        objective=sum(p[i] * x[i] for i in range(6)),
        constraints={0: cast(Constraint, sum(w[i] * x[i] for i in range(6)) <= 47)},
        sense=Instance.MAXIMIZE,
    )


def _progress_snapshot(
    *,
    event: str = "BESTSOLFOUND",
    solving_time_sec: float = 0.25,
    node_count: int = 1,
    total_node_count: int = 1,
    lp_iteration_count: int = 2,
    solution_count: int = 1,
    primal_bound: float = 10.0,
    dual_bound: float = 12.0,
    gap: float = 0.2,
    incumbent_objective: float | None = 10.0,
) -> SCIPProgressSnapshot:
    return SCIPProgressSnapshot(
        event=event,
        solving_time_sec=solving_time_sec,
        node_count=node_count,
        total_node_count=total_node_count,
        lp_iteration_count=lp_iteration_count,
        solution_count=solution_count,
        primal_bound=primal_bound,
        dual_bound=dual_bound,
        gap=gap,
        incumbent_objective=incumbent_objective,
    )


def _termination_report(
    *,
    status: str = "optimal",
    primal_bound: float = 10.0,
    dual_bound: float = 10.0,
    gap: float = 0.0,
    objective_value: float | None = 10.0,
    solving_time_sec: float = 0.75,
) -> SCIPTerminationReport:
    return SCIPTerminationReport(
        status=status,
        primal_bound=primal_bound,
        dual_bound=dual_bound,
        gap=gap,
        objective_value=objective_value,
        node_count=1,
        total_node_count=1,
        lp_iteration_count=2,
        lp_solve_count=1,
        cut_count=0,
        applied_cut_count=0,
        solution_count=1,
        solution_found_count=1,
        best_solution_count=1,
        max_depth=0,
        primal_dual_integral=0.0,
        solving_time_sec=solving_time_sec,
        presolving_time_sec=0.01,
        reading_time_sec=0.0,
        scip_version="9.2.1",
        pyscipopt_version="6.0.0",
    )


def _progress_snapshot_dict(snapshot: SCIPProgressSnapshot) -> dict[str, object]:
    return {
        "event": snapshot.event,
        "solving_time_sec": snapshot.solving_time_sec,
        "node_count": snapshot.node_count,
        "total_node_count": snapshot.total_node_count,
        "lp_iteration_count": snapshot.lp_iteration_count,
        "solution_count": snapshot.solution_count,
        "primal_bound": snapshot.primal_bound,
        "dual_bound": snapshot.dual_bound,
        "gap": snapshot.gap,
        "incumbent_objective": snapshot.incumbent_objective,
    }


def _termination_progress_snapshot_dict(
    report: SCIPTerminationReport,
) -> dict[str, object]:
    return {
        "event": "TERMINATION",
        "solving_time_sec": report.solving_time_sec,
        "node_count": report.node_count,
        "total_node_count": report.total_node_count,
        "lp_iteration_count": report.lp_iteration_count,
        "solution_count": report.solution_count,
        "primal_bound": report.primal_bound,
        "dual_bound": report.dual_bound,
        "gap": report.gap,
        "incumbent_objective": report.objective_value,
    }


def _termination_report_dict(report: SCIPTerminationReport) -> dict[str, object]:
    return {
        "status": report.status,
        "primal_bound": report.primal_bound,
        "dual_bound": report.dual_bound,
        "gap": report.gap,
        "objective_value": report.objective_value,
        "node_count": report.node_count,
        "total_node_count": report.total_node_count,
        "lp_iteration_count": report.lp_iteration_count,
        "lp_solve_count": report.lp_solve_count,
        "cut_count": report.cut_count,
        "applied_cut_count": report.applied_cut_count,
        "solution_count": report.solution_count,
        "solution_found_count": report.solution_found_count,
        "best_solution_count": report.best_solution_count,
        "max_depth": report.max_depth,
        "primal_dual_integral": report.primal_dual_integral,
        "solving_time_sec": report.solving_time_sec,
        "presolving_time_sec": report.presolving_time_sec,
        "reading_time_sec": report.reading_time_sec,
        "scip_version": report.scip_version,
        "pyscipopt_version": report.pyscipopt_version,
    }


def test_solution_optimality():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    y = DecisionVariable.integer(2, lower=0, upper=5)
    ommx_instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(ommx_instance)
    assert solution.optimality == Solution.OPTIMAL


def test_direct_solve_records_termination_report():
    x = DecisionVariable.integer(1, lower=0, upper=5)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    collector = DiagnosticCollector()

    solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)

    assert solution.optimality == Solution.OPTIMAL
    termination_reports = [
        diagnostic
        for diagnostic in collector.diagnostics
        if isinstance(diagnostic, SCIPTerminationReport)
    ]
    assert termination_reports
    assert collector.diagnostics[-1] is termination_reports[-1]

    report = termination_reports[-1]
    assert report.status == "optimal"
    assert report.objective_value == pytest.approx(5.0)
    assert report.gap == pytest.approx(0.0)
    assert report.solution_count >= 1
    assert isinstance(report.pyscipopt_version, str)
    assert isinstance(report.scip_version, str)


def test_direct_solve_records_progress_snapshots():
    instance = _knapsack_instance()
    collector = DiagnosticCollector()

    solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)

    assert solution.optimality == Solution.OPTIMAL
    progress_snapshots = [
        diagnostic
        for diagnostic in collector.diagnostics
        if isinstance(diagnostic, SCIPProgressSnapshot)
    ]
    assert progress_snapshots
    assert {snapshot.event for snapshot in progress_snapshots} <= {
        "BESTSOLFOUND",
        "DUALBOUNDIMPROVED",
        "TERMINATION",
    }
    assert any(snapshot.event == "BESTSOLFOUND" for snapshot in progress_snapshots)
    assert progress_snapshots[-1].event == "TERMINATION"
    assert collector.diagnostics[-2] is progress_snapshots[-1]
    for snapshot in progress_snapshots:
        assert snapshot.solving_time_sec >= 0.0
        assert isinstance(snapshot.primal_bound, float)
        assert isinstance(snapshot.dual_bound, float)
        assert isinstance(snapshot.gap, float)


def test_analyzer_accepts_typed_reports():
    first = _progress_snapshot()
    second = _progress_snapshot(
        event="DUALBOUNDIMPROVED",
        solving_time_sec=0.5,
        primal_bound=10.0,
        dual_bound=10.5,
        gap=0.05,
        incumbent_objective=None,
    )
    termination = _termination_report()

    analyzer = SCIPDiagnosticsAnalyzer([first, second, termination])

    assert analyzer.progress_snapshots == (first, second)
    assert analyzer.progress_history_records == [
        _progress_snapshot_dict(first),
        _progress_snapshot_dict(second),
        _termination_progress_snapshot_dict(termination),
    ]
    assert analyzer.termination_result == _termination_report_dict(termination)
    assert list(analyzer.progress_history_df.columns) == [
        "event",
        "node_count",
        "total_node_count",
        "lp_iteration_count",
        "solution_count",
        "primal_bound",
        "dual_bound",
        "gap",
        "incumbent_objective",
    ]
    assert analyzer.progress_history_df.index.name == "solving_time_sec"
    assert list(analyzer.progress_history_df.index) == [0.25, 0.5, 0.75]
    assert list(analyzer.gap) == [0.2, 0.05, 0.0]
    assert list(analyzer.primal_bound) == [10.0, 10.0, 10.0]
    assert list(analyzer.dual_bound) == [12.0, 10.5, 10.0]
    assert analyzer.incumbent_objective.iloc[0] == 10.0
    assert analyzer.incumbent_objective.iloc[1] is pd.NA
    assert analyzer.incumbent_objective.iloc[2] == 10.0
    assert analyzer.dual_bound.index.name == "solving_time_sec"


def test_analyzer_does_not_duplicate_recorded_termination_snapshot():
    progress = _progress_snapshot()
    termination = _termination_report()
    termination_progress = SCIPProgressSnapshot.from_termination_report(termination)

    analyzer = SCIPDiagnosticsAnalyzer([progress, termination_progress, termination])

    assert analyzer.progress_snapshots == (progress, termination_progress)
    assert analyzer.progress_history_records == [
        _progress_snapshot_dict(progress),
        _termination_progress_snapshot_dict(termination),
    ]


def test_analyzer_does_not_duplicate_serialized_nan_termination_snapshot():
    termination = _termination_report(
        status="infeasible",
        primal_bound=math.inf,
        dual_bound=-math.inf,
        gap=math.nan,
        objective_value=None,
    )
    termination_progress_payload = _termination_progress_snapshot_dict(termination)
    termination_payload = _termination_report_dict(termination)
    termination_progress_payload["gap"] = float("nan")
    termination_payload["gap"] = float("nan")

    analyzer = SCIPDiagnosticsAnalyzer(
        [termination_progress_payload, termination_payload]
    )

    assert len(analyzer.progress_history_records) == 1
    terminal_record = analyzer.progress_history_records[0]
    assert terminal_record["event"] == "TERMINATION"
    assert math.isnan(cast(float, terminal_record["gap"]))


def test_analyzer_accepts_partial_progress_without_termination_row():
    progress = _progress_snapshot()

    analyzer = SCIPDiagnosticsAnalyzer([progress])

    assert analyzer.termination_result is None
    assert analyzer.progress_history_records == [_progress_snapshot_dict(progress)]


def test_analyzer_accepts_experiment_dicts():
    progress = _progress_snapshot()
    termination = _termination_report()

    analyzer = SCIPDiagnosticsAnalyzer(
        [_progress_snapshot_dict(progress), _termination_report_dict(termination)]
    )

    assert analyzer.progress_snapshots == ()
    assert analyzer.progress_history_records == [
        _progress_snapshot_dict(progress),
        _termination_progress_snapshot_dict(termination),
    ]
    assert analyzer.termination_result == _termination_report_dict(termination)


def test_analyzer_dual_bound_series_preserves_infinity_without_runtime_warning():
    analyzer = SCIPDiagnosticsAnalyzer(
        [
            _progress_snapshot(dual_bound=math.inf, incumbent_objective=None),
            _progress_snapshot(solving_time_sec=0.5, dual_bound=10.0),
        ]
    )

    with warnings.catch_warnings():
        warnings.simplefilter("error", RuntimeWarning)
        dual_bound = analyzer.dual_bound

    assert list(dual_bound) == [math.inf, 10.0]
    assert analyzer.incumbent_objective.iloc[0] is pd.NA


def test_progress_snapshot_avoids_callback_get_obj_val_regression():
    """Regression test for the callback-time objective read crash.

    Calling PySCIPOpt's ``Model.getObjVal()`` from a SCIP event callback caused
    a segmentation fault on Python 3.10 in CI. Progress snapshots must read the
    incumbent objective through ``getBestSol()`` and ``getSolObjVal(solution)``
    instead.
    """

    class FakeSolution:
        pass

    class FakeModel:
        def __init__(self) -> None:
            self.solution = FakeSolution()

        def getNSols(self) -> int:
            return 1

        def getPrimalbound(self) -> float:
            return 12.5

        def getSolvingTime(self) -> float:
            return 0.25

        def getNNodes(self) -> int:
            return 2

        def getNTotalNodes(self) -> int:
            return 3

        def getNLPIterations(self) -> int:
            return 4

        def getDualbound(self) -> float:
            return 13.0

        def getGap(self) -> float:
            return 0.04

        def getObjVal(self) -> float:
            # This is the callback-time API path that crashed with a real SCIP
            # model, so the regression test makes any use of it fail loudly.
            raise AssertionError("getObjVal must not be called from an event")

        def getBestSol(self) -> FakeSolution:
            return self.solution

        def getSolObjVal(self, solution: FakeSolution) -> float:
            assert solution is self.solution
            return 14.0

    class FakeEvent:
        def getName(self) -> str:
            return "BESTSOLFOUND"

    snapshot = SCIPProgressSnapshot.from_event(
        cast(Any, FakeModel()), cast(Any, FakeEvent())
    )

    assert snapshot.event == "BESTSOLFOUND"
    assert snapshot.primal_bound == pytest.approx(12.5)
    assert snapshot.incumbent_objective == pytest.approx(14.0)


def test_progress_snapshot_can_be_derived_from_termination_report():
    termination = _termination_report()

    snapshot = SCIPProgressSnapshot.from_termination_report(termination)

    assert _progress_snapshot_dict(snapshot) == _termination_progress_snapshot_dict(
        termination
    )


def test_experiment_stores_diagnostics_as_dict_payload():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_solve(
                OMMXPySCIPOptAdapter,
                _knapsack_instance(),
                store_diagnostics=True,
            )

    diagnostics = experiment.runs[0].solves[0].diagnostics
    analyzer = SCIPDiagnosticsAnalyzer(diagnostics)

    assert diagnostics
    assert all(isinstance(diagnostic, dict) for diagnostic in diagnostics)
    assert diagnostics[-1]["status"] == "optimal"
    assert analyzer.progress_history_records
    assert {record["event"] for record in analyzer.progress_history_records} <= {
        "BESTSOLFOUND",
        "DUALBOUNDIMPROVED",
        "TERMINATION",
    }
    assert analyzer.progress_history_records[-1]["event"] == "TERMINATION"
    assert analyzer.termination_result is not None
    assert analyzer.termination_result["status"] == "optimal"


def test_direct_collector_keeps_termination_report_before_decode_error():
    x = DecisionVariable.integer(1, lower=0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )
    collector = DiagnosticCollector()

    with pytest.raises(UnboundedDetected):
        OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)

    diagnostic = collector.diagnostics[-1]
    assert isinstance(diagnostic, SCIPTerminationReport)
    assert diagnostic.status == "unbounded"


def test_scip_termination_report_preserves_non_finite_bounds():
    class FakeModel:
        def getNSols(self) -> int:
            return 0

        def getStatus(self) -> str:
            return "infeasible"

        def getPrimalbound(self) -> float:
            return math.inf

        def getDualbound(self) -> float:
            return -math.inf

        def getGap(self) -> float:
            return math.nan

        def getNNodes(self) -> int:
            return 0

        def getNTotalNodes(self) -> int:
            return 0

        def getNLPIterations(self) -> int:
            return 0

        def getNLPs(self) -> int:
            return 0

        def getNCuts(self) -> int:
            return 0

        def getNCutsApplied(self) -> int:
            return 0

        def getNSolsFound(self) -> int:
            return 0

        def getNBestSolsFound(self) -> int:
            return 0

        def getMaxDepth(self) -> int:
            return 0

        def getPrimalDualIntegral(self) -> float:
            return 0.0

        def getSolvingTime(self) -> float:
            return 1.25

        def getPresolvingTime(self) -> float:
            return 0.25

        def getReadingTime(self) -> float:
            return 0.0

        def getMajorVersion(self) -> int:
            return 9

        def getMinorVersion(self) -> int:
            return 2

        def getTechVersion(self) -> int:
            return 1

    report = SCIPTerminationReport.from_model(cast(Any, FakeModel()))

    assert math.isinf(report.primal_bound)
    assert report.primal_bound > 0
    assert math.isinf(report.dual_bound)
    assert report.dual_bound < 0
    assert math.isnan(report.gap)
    assert report.objective_value is None


def test_scip_termination_report_rejects_unoptimized_model():
    class FakeModel:
        def getStatus(self) -> str:
            return "unknown"

        def getNSols(self) -> int:
            raise AssertionError(
                "from_model should reject unknown before reading solve results"
            )

    with pytest.raises(
        OMMXPySCIPOptAdapterError,
        match=r"The model may not be optimized\. \[status: unknown\]",
    ):
        SCIPTerminationReport.from_model(cast(Any, FakeModel()))
