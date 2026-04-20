"""Verify that the eight Plant Placement builders define the same optimum."""

from __future__ import annotations

import math
import random

import pytest

from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

from ommx.testing.placement import (
    Input,
    build_bigm,
    build_sos1,
    build_sos1_on_both_with_delta,
    build_sos1_on_both_with_delta_with_card,
    build_sos1_on_c_with_delta,
    build_sos1_on_c_with_delta_with_card,
    build_sos1_on_delta,
    build_sos1_on_delta_with_card,
)


@pytest.fixture
def small_input() -> Input:
    random.seed(42)
    return Input.random(num_plants=6, num_clients=10)


def test_all_formulations_match(small_input: Input) -> None:
    builders = {
        "sos1": build_sos1,
        "sos1_on_c_with_delta": build_sos1_on_c_with_delta,
        "sos1_on_c_with_delta_with_card": build_sos1_on_c_with_delta_with_card,
        "sos1_on_delta": build_sos1_on_delta,
        "sos1_on_delta_with_card": build_sos1_on_delta_with_card,
        "sos1_on_both_with_delta": build_sos1_on_both_with_delta,
        "sos1_on_both_with_delta_with_card": build_sos1_on_both_with_delta_with_card,
        "bigm": build_bigm,
    }
    objectives = {}
    for name, builder in builders.items():
        sol = OMMXPySCIPOptAdapter.solve(builder(small_input))
        assert sol.feasible, f"{name} should be feasible"
        objectives[name] = sol.objective

    base = objectives["sos1"]
    for name, value in objectives.items():
        assert math.isclose(value, base, rel_tol=1e-6), (
            f"{name} optimum {value} differs from sos1 optimum {base}"
        )
