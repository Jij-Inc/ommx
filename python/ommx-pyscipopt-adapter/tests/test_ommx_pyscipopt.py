import pytest

from ommx.adapter import DiagnosticCollector
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
    assert report.solving_time_sec is None or report.solving_time_sec >= 0.0
