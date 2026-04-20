"""Benchmark four Plant Placement Problem formulations end-to-end through the adapter."""

from __future__ import annotations

import random
from typing import List

import pytest

from ommx.v1 import Instance
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

from ommx.testing.placement import (
    Input,
    build_bigm,
    build_sos1,
    build_sos1_on_delta,
    build_sos1_on_delta_with_card,
)

_SIZES = [(6 * (i + 1), 10 * (i + 1)) for i in range(5)]
_INSTANCES_PER_SIZE = 3


@pytest.fixture(
    scope="session",
    params=_SIZES,
    ids=lambda pc: f"plants={pc[0]:02d}-clients={pc[1]:03d}",
)
def placement_inputs(request: pytest.FixtureRequest) -> List[Input]:
    num_plants, num_clients = request.param
    random.seed(42)
    return [
        Input.random(num_plants=num_plants, num_clients=num_clients)
        for _ in range(_INSTANCES_PER_SIZE)
    ]


@pytest.fixture(scope="session")
def sos1_instances(placement_inputs: List[Input]) -> List[Instance]:
    return [build_sos1(inp) for inp in placement_inputs]


@pytest.fixture(scope="session")
def sos1_on_delta_instances(placement_inputs: List[Input]) -> List[Instance]:
    return [build_sos1_on_delta(inp) for inp in placement_inputs]


@pytest.fixture(scope="session")
def sos1_on_delta_with_card_instances(
    placement_inputs: List[Input],
) -> List[Instance]:
    return [build_sos1_on_delta_with_card(inp) for inp in placement_inputs]


@pytest.fixture(scope="session")
def bigm_instances(placement_inputs: List[Input]) -> List[Instance]:
    return [build_bigm(inp) for inp in placement_inputs]


def _solve_all(instances: List[Instance]) -> None:
    for inst in instances:
        OMMXPySCIPOptAdapter.solve(inst)


@pytest.mark.benchmark
def test_bench_sos1(benchmark, sos1_instances: List[Instance]) -> None:
    benchmark(_solve_all, sos1_instances)


@pytest.mark.benchmark
def test_bench_sos1_on_delta(
    benchmark, sos1_on_delta_instances: List[Instance]
) -> None:
    benchmark(_solve_all, sos1_on_delta_instances)


@pytest.mark.benchmark
def test_bench_sos1_on_delta_with_card(
    benchmark, sos1_on_delta_with_card_instances: List[Instance]
) -> None:
    benchmark(_solve_all, sos1_on_delta_with_card_instances)


@pytest.mark.benchmark
def test_bench_bigm(benchmark, bigm_instances: List[Instance]) -> None:
    benchmark(_solve_all, bigm_instances)
