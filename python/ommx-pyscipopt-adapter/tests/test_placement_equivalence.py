"""Verify that ``build_sos1`` and ``build_bigm`` define the same optimum."""

from __future__ import annotations

import math
import random

import pytest

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

from ommx.testing.placement import Input, build_bigm, build_sos1


@pytest.fixture
def small_input() -> Input:
    random.seed(42)
    return Input.random(num_plants=6, num_clients=10)


def test_sos1_and_bigm_match(small_input: Input) -> None:
    sos1_sol = OMMXPySCIPOptAdapter.solve(build_sos1(small_input))
    bigm_sol = OMMXPySCIPOptAdapter.solve(build_bigm(small_input))

    assert sos1_sol.feasible
    assert bigm_sol.feasible
    assert math.isclose(sos1_sol.objective, bigm_sol.objective, rel_tol=1e-6)
