from collections.abc import Callable
from pathlib import Path

import pytest

from ommx import qplib
from ommx.v1 import Instance


FIXTURE = (
    Path(__file__).resolve().parents[3]
    / "rust/ommx/tests/fixtures/quadratic_scaling.qplib"
)


@pytest.mark.parametrize("loader", [Instance.load_qplib, qplib.load_file])
@pytest.mark.parametrize(
    "a,b",
    [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (0.5, -0.75), (-1, 2)],
)
def test_qplib_preserves_objective_and_constraint_values(
    loader: Callable[[str], Instance], a: float, b: float
) -> None:
    instance = loader(str(FIXTURE))
    state = {0: a, 1: b}
    expected = a * a + 3 * a * b - 2 * b * b + 5 * a - 7 * b + 11
    assert instance.objective.evaluate(state) == expected

    constraints = {constraint.id: constraint for constraint in instance.constraints}
    assert set(constraints) == {0, 1}
    g = 2 * a * a - 5 * a * b + 3 * b * b + 13 * a + 17 * b
    assert constraints[0].function.evaluate(state) == g - 23
    assert constraints[1].function.evaluate(state) == -19 - g
