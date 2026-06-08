import math
from typing import Any, cast

import pytest

from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx.experiment import Experiment
from ommx.v1 import Instance, Constraint, DecisionVariable, Solution

from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapter,
    SCIPProgressReport,
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


def test_solve_records_termination_diagnostics():
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
    diagnostic = collector.diagnostics[-1]
    assert isinstance(diagnostic, SCIPTerminationReport)
    report = diagnostic
    assert report.status == "optimal"
    assert report.objective_value == pytest.approx(5.0)
    assert report.gap == pytest.approx(0.0)
    assert report.solution_count >= 1
    assert report.solution_found_count >= 1
    assert report.best_solution_count >= 1
    assert report.node_count >= 0
    assert report.total_node_count >= report.node_count
    assert report.lp_iteration_count >= 0
    assert report.lp_solve_count >= 0
    assert report.cut_count >= 0
    assert report.applied_cut_count >= 0
    assert report.max_depth >= -1
    assert report.primal_dual_integral >= 0.0
    assert isinstance(report.pyscipopt_version, str)
    assert isinstance(report.scip_version, str)
    assert report.solving_time_sec >= 0.0
    assert report.presolving_time_sec >= 0.0
    assert report.reading_time_sec >= 0.0


def test_solve_records_progress_diagnostics():
    instance = _knapsack_instance()
    collector = DiagnosticCollector()

    solution = OMMXPySCIPOptAdapter.solve(instance, diagnostics=collector)

    assert solution.optimality == Solution.OPTIMAL
    progress_reports = [
        diagnostic
        for diagnostic in collector.diagnostics
        if isinstance(diagnostic, SCIPProgressReport)
    ]
    (progress_report,) = progress_reports
    assert progress_report.snapshots
    assert {snapshot.event for snapshot in progress_report.snapshots} <= {
        "BESTSOLFOUND",
        "DUALBOUNDIMPROVED",
    }
    assert any(
        snapshot.event == "BESTSOLFOUND" for snapshot in progress_report.snapshots
    )
    for snapshot in progress_report.snapshots:
        assert snapshot.solving_time_sec >= 0.0
        assert snapshot.node_count >= 0
        assert snapshot.total_node_count >= snapshot.node_count
        assert snapshot.lp_iteration_count >= 0
        assert snapshot.solution_count >= 0
        assert isinstance(snapshot.primal_bound, float)
        assert isinstance(snapshot.dual_bound, float)
        assert isinstance(snapshot.gap, float)


def test_log_solve_stores_progress_diagnostics():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_solve(OMMXPySCIPOptAdapter, _knapsack_instance())

    progress_report, termination_report = experiment.runs[0].solves[0].diagnostics

    assert progress_report["snapshots"]
    assert progress_report["snapshots"][0]["event"] in {
        "BESTSOLFOUND",
        "DUALBOUNDIMPROVED",
    }
    assert "incumbent_objective" in progress_report["snapshots"][0]
    assert termination_report["status"] == "optimal"
    assert termination_report["lp_iteration_count"] >= 0


def test_solve_records_termination_diagnostics_before_decode_errors():
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
