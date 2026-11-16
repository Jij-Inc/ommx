from unittest.mock import Mock

import highspy
import pytest

from ommx import Instance, Sense
from ommx.adapter import DiagnosticCollector
from ommx_highs_adapter import OMMXHighsAdapter, OMMXHighsAdapterError


def constant_instance():
    return Instance.from_components(
        decision_variables=[], objective=7, active_constraints={}, sense=Sense.Minimize
    )


@pytest.mark.parametrize("prepare", [False, True])
@pytest.mark.parametrize("collect_diagnostics", [False, True])
def test_run_error_is_reported_without_decoding(
    monkeypatch, prepare, collect_diagnostics
):
    monkeypatch.setattr(highspy.Highs, "run", lambda self: highspy.HighsStatus.kError)
    decode = Mock(side_effect=AssertionError("A failed run must not be decoded"))
    monkeypatch.setattr(OMMXHighsAdapter, "decode", decode)
    solve = (
        OMMXHighsAdapter.solve
        if prepare
        else OMMXHighsAdapter.solve_without_preparation
    )
    diagnostics = DiagnosticCollector() if collect_diagnostics else None
    with pytest.raises(OMMXHighsAdapterError, match="HiGHS run failed.*kError"):
        solve(constant_instance(), diagnostics=diagnostics)
    decode.assert_not_called()


def test_run_warning_can_still_return_a_solution(monkeypatch):
    original_run = highspy.Highs.run

    def run_with_warning(model):
        assert original_run(model) == highspy.HighsStatus.kOk
        return highspy.HighsStatus.kWarning

    monkeypatch.setattr(highspy.Highs, "run", run_with_warning)
    solution = OMMXHighsAdapter.solve_without_preparation(constant_instance())
    assert solution.feasible
    assert solution.objective == 7


@pytest.mark.parametrize(
    "status",
    [
        highspy.HighsModelStatus.kLoadError,
        highspy.HighsModelStatus.kModelError,
        highspy.HighsModelStatus.kPresolveError,
        highspy.HighsModelStatus.kSolveError,
        highspy.HighsModelStatus.kPostsolveError,
        highspy.HighsModelStatus.kUnknown,
    ],
)
@pytest.mark.parametrize("decode_state", [False, True])
def test_decode_solver_failure_does_not_read_solution(status, decode_state):
    adapter = OMMXHighsAdapter(constant_instance())
    model = Mock(spec=highspy.Highs)
    model.getModelStatus.return_value = status
    decode = adapter.decode_to_state if decode_state else adapter.decode
    with pytest.raises(OMMXHighsAdapterError, match="HiGHS failed to solve the model"):
        decode(model)
    model.getSolution.assert_not_called()
    model.run.assert_not_called()
