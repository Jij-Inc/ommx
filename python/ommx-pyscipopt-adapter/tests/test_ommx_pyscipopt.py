import json

import pytest

from ommx.adapter import DiagnosticCollector
from ommx.v1 import Instance, DecisionVariable, Solution

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


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
    (entry,) = collector.entries
    assert entry.name == "solver/scip/termination-report"
    assert entry.media_type == "application/json"
    assert entry.annotations == {
        "org.ommx.solver.name": "scip",
        "org.ommx.solver.diagnostic.kind": "termination_report",
        "org.ommx.solver.diagnostic.schema": (
            "org.ommx.solver.scip.termination-report.v1"
        ),
    }
    report = json.loads(entry.data)
    assert report["schema"] == "org.ommx.solver.scip.termination-report.v1"
    assert report["solver"] == "scip"
    assert report["adapter"] == "ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter"
    assert report["status"] == "optimal"
    assert report["objective_value"] == pytest.approx(5.0)
    assert report["gap"] == pytest.approx(0.0)
    assert report["solution_count"] >= 1
    assert report["node_count"] >= 0
    assert isinstance(report["pyscipopt_version"], str)
    assert isinstance(report["scip_version"], str)
