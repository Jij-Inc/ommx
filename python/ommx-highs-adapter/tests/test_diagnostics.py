from dataclasses import asdict
from typing import cast

import highspy
import pytest

from ommx.adapter import DiagnosticCollector, InfeasibleDetected
from ommx.experiment import Experiment
from ommx.v1 import Constraint, DecisionVariable, Instance, Solution
from ommx_highs_adapter import (
    HighsDiagnosticsAnalyzer,
    HighsProgressSnapshot,
    HighsTerminationReport,
    OMMXHighsAdapter,
    OMMXHighsAdapterError,
)


def _mip_instance() -> Instance:
    x = DecisionVariable.integer(1, lower=0, upper=5)
    return Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )


def _knapsack_instance() -> Instance:
    values = [i % 11 + 1 for i in range(30)]
    weights = [i % 7 + 1 for i in range(30)]
    x = [DecisionVariable.binary(i) for i in range(30)]
    return Instance.from_components(
        decision_variables=x,
        objective=sum(values[i] * x[i] for i in range(30)),
        constraints={
            0: cast(Constraint, sum(weights[i] * x[i] for i in range(30)) <= 60)
        },
        sense=Instance.MAXIMIZE,
    )


def _infeasible_instance() -> Instance:
    x = DecisionVariable.continuous(1)
    return Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={0: x == 0, 1: x == 1},
        sense=Instance.MINIMIZE,
    )


def _progress_snapshot(
    *,
    event: str = "MIP logging",
    solving_time_sec: float = 0.25,
    mip_node_count: int = 0,
    objective_value: float = 10.0,
    primal_bound: float = 10.0,
    dual_bound: float = 12.0,
    gap: float = 0.2,
) -> HighsProgressSnapshot:
    return HighsProgressSnapshot(
        event=event,
        solving_time_sec=solving_time_sec,
        mip_node_count=mip_node_count,
        simplex_iteration_count=-1,
        ipm_iteration_count=-1,
        pdlp_iteration_count=-1,
        objective_value=objective_value,
        primal_bound=primal_bound,
        dual_bound=dual_bound,
        gap=gap,
    )


def _termination_report() -> HighsTerminationReport:
    return HighsTerminationReport(
        status="Optimal",
        objective_value=5.0,
        mip_dual_bound=5.0,
        mip_gap=0.0,
        mip_node_count=0,
        simplex_iteration_count=0,
        ipm_iteration_count=-1,
        crossover_iteration_count=-1,
        pdlp_iteration_count=-1,
        primal_dual_integral=0.0,
        primal_solution_status=2,
        dual_solution_status=0,
        max_integrality_violation=0.0,
        max_primal_infeasibility=0.0,
        max_dual_infeasibility=0.0,
        run_time_sec=0.01,
        highs_version="1.14.0",
        highs_githash="7df0786",
    )


def _progress_snapshot_dict(snapshot: HighsProgressSnapshot) -> dict[str, object]:
    return asdict(snapshot)


def test_direct_solve_records_termination_report():
    collector = DiagnosticCollector()

    solution = OMMXHighsAdapter.solve(_mip_instance(), diagnostics=collector)

    assert solution.optimality == Solution.OPTIMAL
    reports = [
        diagnostic
        for diagnostic in collector.diagnostics
        if isinstance(diagnostic, HighsTerminationReport)
    ]
    assert reports
    assert collector.diagnostics[-1] is reports[-1]

    report = reports[-1]
    assert report.status == "Optimal"
    assert report.objective_value == pytest.approx(5.0)
    assert report.mip_dual_bound == pytest.approx(5.0)
    assert report.mip_gap == pytest.approx(0.0)
    assert report.mip_node_count >= 0
    assert isinstance(report.highs_version, str)
    assert isinstance(report.highs_githash, str)


def test_direct_solve_records_progress_snapshots():
    collector = DiagnosticCollector()

    solution = OMMXHighsAdapter.solve(_knapsack_instance(), diagnostics=collector)

    assert solution.optimality == Solution.OPTIMAL
    progress_snapshots = [
        diagnostic
        for diagnostic in collector.diagnostics
        if isinstance(diagnostic, HighsProgressSnapshot)
    ]
    assert progress_snapshots
    assert collector.diagnostics[-1].__class__ is HighsTerminationReport
    assert {snapshot.event for snapshot in progress_snapshots} == {"MIP logging"}
    for snapshot in progress_snapshots:
        assert snapshot.solving_time_sec >= 0.0
        assert snapshot.mip_node_count >= 0
        assert isinstance(snapshot.primal_bound, float)
        assert isinstance(snapshot.dual_bound, float)
        assert isinstance(snapshot.gap, float)


def test_analyzer_accepts_typed_reports():
    first = _progress_snapshot()
    second = _progress_snapshot(
        solving_time_sec=0.5,
        mip_node_count=1,
        objective_value=10.0,
        primal_bound=10.0,
        dual_bound=10.0,
        gap=0.0,
    )
    report = _termination_report()

    analyzer = HighsDiagnosticsAnalyzer([first, second, report])

    assert analyzer.progress_snapshots == (first, second)
    assert analyzer.progress_history_records == [
        _progress_snapshot_dict(first),
        _progress_snapshot_dict(second),
    ]
    assert analyzer.termination_result == asdict(report)
    assert list(analyzer.progress_history_df.columns) == [
        "event",
        "mip_node_count",
        "simplex_iteration_count",
        "ipm_iteration_count",
        "pdlp_iteration_count",
        "objective_value",
        "primal_bound",
        "dual_bound",
        "gap",
    ]
    assert analyzer.progress_history_df.index.name == "solving_time_sec"
    assert list(analyzer.progress_history_df.index) == [0.25, 0.5]
    assert list(analyzer.dual_bound) == [12.0, 10.0]
    assert list(analyzer.gap) == [0.2, 0.0]
    assert list(analyzer.primal_bound) == [10.0, 10.0]
    assert list(analyzer.node_count) == [0, 1]
    assert analyzer.dual_bound.index.name == "solving_time_sec"
    assert analyzer.termination_status == "Optimal"
    assert analyzer.termination_objective_value == pytest.approx(5.0)
    assert analyzer.termination_mip_dual_bound == pytest.approx(5.0)
    assert analyzer.termination_mip_gap == pytest.approx(0.0)
    assert analyzer.termination_mip_node_count == 0


def test_analyzer_accepts_experiment_dicts():
    progress = _progress_snapshot()
    report = _termination_report()
    progress_payload = asdict(progress)
    payload = asdict(report)

    analyzer = HighsDiagnosticsAnalyzer([progress_payload, payload])

    assert analyzer.progress_snapshots == ()
    assert analyzer.progress_history_records == [progress_payload]
    assert analyzer.termination_result == payload
    assert list(analyzer.dual_bound) == [12.0]
    assert analyzer.termination_mip_dual_bound == pytest.approx(5.0)


def test_experiment_stores_diagnostics_as_dict_payload():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_solve(
                OMMXHighsAdapter,
                _knapsack_instance(),
                store_diagnostics=True,
            )

    diagnostics = experiment.runs[0].solves[0].diagnostics
    analyzer = HighsDiagnosticsAnalyzer(diagnostics)

    assert diagnostics
    assert all(isinstance(diagnostic, dict) for diagnostic in diagnostics)
    assert analyzer.progress_history_records
    assert diagnostics[-1]["status"] == "Optimal"
    assert analyzer.termination_result is not None
    assert analyzer.termination_result["mip_gap"] == pytest.approx(0.0)
    assert analyzer.dual_bound.index.name == "solving_time_sec"


def test_direct_collector_keeps_termination_report_before_decode_error():
    collector = DiagnosticCollector()

    with pytest.raises(InfeasibleDetected):
        OMMXHighsAdapter.solve(_infeasible_instance(), diagnostics=collector)

    diagnostic = collector.diagnostics[-1]
    assert isinstance(diagnostic, HighsTerminationReport)
    assert diagnostic.status == "Infeasible"
    assert diagnostic.objective_value is None


def test_highs_termination_report_rejects_unoptimized_model():
    model = highspy.Highs()

    with pytest.raises(
        OMMXHighsAdapterError,
        match=r"The model may not be optimized\. \[status: Not Set\]",
    ):
        HighsTerminationReport.from_model(model)
