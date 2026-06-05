import math
from typing import Any, cast

import pytest

from ommx.adapter import DiagnosticCollector, UnboundedDetected
from ommx.v1 import Instance, DecisionVariable, Solution

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, SCIPTerminationReport


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
    (diagnostic,) = collector.diagnostics
    assert isinstance(diagnostic, SCIPTerminationReport)
    report = diagnostic
    assert report.status == "optimal"
    assert report.objective_value == pytest.approx(5.0)
    assert report.gap == pytest.approx(0.0)
    assert report.solution_count >= 1
    assert report.node_count >= 0
    assert isinstance(report.pyscipopt_version, str)
    assert isinstance(report.scip_version, str)
    assert report.solving_time_sec >= 0.0


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

    (diagnostic,) = collector.diagnostics
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

        def getSolvingTime(self) -> float:
            return 1.25

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
