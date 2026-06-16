from dataclasses import asdict

import highspy
import pytest

from ommx.adapter import DiagnosticCollector, InfeasibleDetected
from ommx.experiment import Experiment
from ommx.v1 import DecisionVariable, Instance, Solution
from ommx_highs_adapter import (
    HighsDiagnosticsAnalyzer,
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


def _infeasible_instance() -> Instance:
    x = DecisionVariable.continuous(1)
    return Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={0: x == 0, 1: x == 1},
        sense=Instance.MINIMIZE,
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


def test_analyzer_accepts_typed_reports():
    report = _termination_report()

    analyzer = HighsDiagnosticsAnalyzer([report])

    assert analyzer.termination_result == asdict(report)
    assert analyzer.status == "Optimal"
    assert analyzer.objective_value == pytest.approx(5.0)
    assert analyzer.mip_dual_bound == pytest.approx(5.0)
    assert analyzer.dual_bound == pytest.approx(5.0)
    assert analyzer.mip_gap == pytest.approx(0.0)
    assert analyzer.gap == pytest.approx(0.0)
    assert analyzer.mip_node_count == 0
    assert analyzer.node_count == 0


def test_analyzer_accepts_experiment_dicts():
    report = _termination_report()
    payload = asdict(report)

    analyzer = HighsDiagnosticsAnalyzer([payload])

    assert analyzer.termination_result == payload
    assert analyzer.dual_bound == pytest.approx(5.0)


def test_experiment_stores_diagnostics_as_dict_payload():
    with Experiment.with_temp_local_registry() as experiment:
        with experiment.run() as run:
            run.log_solve(
                OMMXHighsAdapter,
                _mip_instance(),
                store_diagnostics=True,
            )

    diagnostics = experiment.runs[0].solves[0].diagnostics
    analyzer = HighsDiagnosticsAnalyzer(diagnostics)

    assert diagnostics
    assert all(isinstance(diagnostic, dict) for diagnostic in diagnostics)
    assert diagnostics[-1]["status"] == "Optimal"
    assert analyzer.dual_bound == pytest.approx(5.0)
    assert analyzer.gap == pytest.approx(0.0)


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
