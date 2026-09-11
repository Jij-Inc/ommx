from pathlib import Path

import pytest

from ommx import Instance, State, qplib


FIXTURES = Path(__file__).resolve().parents[3] / "rust/ommx/tests/fixtures"


@pytest.mark.parametrize(
    ("tag", "expected"),
    [("0018", -6.386014981598350), ("0681", 45.244448166487501)],
)
def test_published_qplib_solution(tag: str, expected: float) -> None:
    instance = Instance.load_qplib(str(FIXTURES / f"QPLIB_{tag}.qplib"))
    state = State.load_qplib_solution(
        str(FIXTURES / f"QPLIB_{tag}.sol"),
        num_variables=len(instance.decision_variables),
    )
    assert isinstance(state, State)
    assert set(state.entries) == {
        variable.id for variable in instance.decision_variables
    }
    solution = instance.evaluate(state, atol=1e-8)
    assert solution.objective == pytest.approx(expected, abs=1e-10, rel=0)
    assert solution.feasible


def test_solution_loader_fills_omitted_variables(tmp_path: Path) -> None:
    path = tmp_path / "solution.sol"
    path.write_text("objvar -12\nx2 0.5\nb4 1\ni5 -2\n")
    state = qplib.load_solution(str(path), num_variables=5)
    assert state.entries == {0: 0.5, 1: 0.0, 2: 1.0, 3: -2.0, 4: 0.0}


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ("# comment\nx2\n", "line 2: expected a variable name and a numeric value"),
        ("x2 1\nb2 1\n", "line 2: duplicate variable ID 0"),
        ("x5 1\n", "line 1: .*outside 0..3"),
        ("x2 NaN\n", "line 1: solution values must be finite"),
        ("custom_name 1\n", "line 1: invalid variable name"),
    ],
)
def test_solution_errors_are_value_errors(
    tmp_path: Path, text: str, message: str
) -> None:
    path = tmp_path / "invalid.sol"
    path.write_text(text)
    with pytest.raises(ValueError, match=message):
        State.load_qplib_solution(str(path), num_variables=3)


def test_missing_solution_file_is_runtime_error(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="Failed to read QPLIB solution"):
        State.load_qplib_solution(str(tmp_path / "missing.sol"), num_variables=3)
