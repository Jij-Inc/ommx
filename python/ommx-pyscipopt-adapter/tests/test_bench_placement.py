"""Benchmark eight Plant Placement Problem formulations through SCIP only.

Each ``placement_inputs`` parameterisation is converted to ``ommx.v1.Instance``,
then to ``pyscipopt.Model``, in session-scoped fixtures — the OMMX construction
and the OMMX → SCIP translation are *not* in the measurement. Each benchmark
calls ``model.freeTransform()`` to discard SCIP's transformed problem from any
previous run, then ``model.optimize()`` to re-run presolve and
branch-and-bound. The reported time is therefore SCIP's own processing time,
isolated from adapter overhead.
"""

from __future__ import annotations

import random
from typing import Callable, List

import pyscipopt
import pytest

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
from ommx.v1 import Instance
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

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


def _build_models(
    inputs: List[Input], builder: Callable[[Input], Instance]
) -> List[pyscipopt.Model]:
    return [OMMXPySCIPOptAdapter(builder(inp)).solver_input for inp in inputs]


@pytest.fixture(scope="session")
def sos1_models(placement_inputs: List[Input]) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1)


@pytest.fixture(scope="session")
def sos1_on_c_with_delta_models(
    placement_inputs: List[Input],
) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_c_with_delta)


@pytest.fixture(scope="session")
def sos1_on_c_with_delta_with_card_models(
    placement_inputs: List[Input],
) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_c_with_delta_with_card)


@pytest.fixture(scope="session")
def sos1_on_delta_models(placement_inputs: List[Input]) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_delta)


@pytest.fixture(scope="session")
def sos1_on_delta_with_card_models(
    placement_inputs: List[Input],
) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_delta_with_card)


@pytest.fixture(scope="session")
def sos1_on_both_with_delta_models(
    placement_inputs: List[Input],
) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_both_with_delta)


@pytest.fixture(scope="session")
def sos1_on_both_with_delta_with_card_models(
    placement_inputs: List[Input],
) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_sos1_on_both_with_delta_with_card)


@pytest.fixture(scope="session")
def bigm_models(placement_inputs: List[Input]) -> List[pyscipopt.Model]:
    return _build_models(placement_inputs, build_bigm)


def _optimize_all(models: List[pyscipopt.Model]) -> None:
    for m in models:
        m.freeTransform()
        m.optimize()


@pytest.mark.benchmark
def test_bench_sos1(benchmark, sos1_models: List[pyscipopt.Model]) -> None:
    benchmark(_optimize_all, sos1_models)


@pytest.mark.benchmark
def test_bench_sos1_on_c_with_delta(
    benchmark, sos1_on_c_with_delta_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_c_with_delta_models)


@pytest.mark.benchmark
def test_bench_sos1_on_c_with_delta_with_card(
    benchmark, sos1_on_c_with_delta_with_card_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_c_with_delta_with_card_models)


@pytest.mark.benchmark
def test_bench_sos1_on_delta(
    benchmark, sos1_on_delta_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_delta_models)


@pytest.mark.benchmark
def test_bench_sos1_on_delta_with_card(
    benchmark, sos1_on_delta_with_card_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_delta_with_card_models)


@pytest.mark.benchmark
def test_bench_sos1_on_both_with_delta(
    benchmark, sos1_on_both_with_delta_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_both_with_delta_models)


@pytest.mark.benchmark
def test_bench_sos1_on_both_with_delta_with_card(
    benchmark, sos1_on_both_with_delta_with_card_models: List[pyscipopt.Model]
) -> None:
    benchmark(_optimize_all, sos1_on_both_with_delta_with_card_models)


@pytest.mark.benchmark
def test_bench_bigm(benchmark, bigm_models: List[pyscipopt.Model]) -> None:
    benchmark(_optimize_all, bigm_models)
