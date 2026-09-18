"""Published Rust SDK v3 producer sending V1 models to the real Python SDK 2.9."""

from importlib.metadata import version
from itertools import product
from pathlib import Path
import tomllib

import bridge_test_modeling as modeling
import ommx
import pytest
from ommx import v1


def test_receiver_distribution():
    sdk_manifest = Path(__file__).parents[1] / "ommx" / "pyproject.toml"
    expected_version = tomllib.loads(sdk_manifest.read_text())["project"]["version"]
    assert version("ommx") == expected_version
    assert ommx.Instance is v1.Instance


@pytest.mark.parametrize("costs", [[3.0, 1.0, 2.0], [-2.0, 0.0], [0.0]])
def test_one_hot_payload_and_evaluation(costs: list[float]):
    instance = modeling.compile_one_hot(costs)
    assert type(instance) is ommx.Instance
    assert instance.sense == ommx.Instance.MINIMIZE
    assert [x.id for x in instance.decision_variables] == list(range(len(costs)))
    assert [(x.name, x.subscripts) for x in instance.decision_variables] == [
        ("x", [i]) for i in range(len(costs))
    ]
    assert [(row.id, row.name) for row in instance.constraints] == [(23, "one_hot")]
    assert instance.constraint_hints.one_hot_constraints == [
        v1.OneHot(id=23, variables=list(range(len(costs))))
    ]
    assert not instance.constraint_hints.sos1_constraints
    assert not list(instance.removed_constraints)
    for values in product([0, 1], repeat=len(costs)):
        solution = instance.evaluate(dict(enumerate(values)))
        assert solution.feasible == (sum(values) == 1)
        assert solution.objective == pytest.approx(
            sum(c * x for c, x in zip(costs, values))
        )


@pytest.mark.parametrize(
    "bounds",
    [
        [(-2.0, 3.0), (0.0, 4.0)],
        [(-2.0, 0.0), (-3.0, 0.0)],
        [(-2.0, 3.0), (0.0, 0.0), (0.0, 4.0)],
        [(1.0, 2.0), (-3.0, 0.0)],
        [(0.0, 0.0), (0.0, 4.0)],
        [(0.0, 0.0), (0.0, 0.0)],
    ],
)
def test_sos1_hints_and_projected_feasible_set(bounds: list[tuple[float, float]]):
    costs = [float(i + 1) for i in range(len(bounds))]
    instance = modeling.compile_sos1(costs, bounds)
    assert type(instance) is ommx.Instance
    active = [i for i, bound in enumerate(bounds) if bound != (0.0, 0.0)]
    selectors = [len(bounds) + i for i in active] if len(active) >= 2 else []
    assert [x.id for x in instance.decision_variables] == list(
        range(len(bounds))
    ) + selectors
    for variable, (lower, upper) in zip(instance.decision_variables, bounds):
        assert variable.name == "x"
        assert variable.subscripts == [variable.id]
        assert (variable.bound.lower, variable.bound.upper) == (lower, upper)

    assert not instance.constraint_hints.one_hot_constraints
    assert not list(instance.removed_constraints)
    if selectors:
        links = sorted(
            [100 + 2 * i for i in active if bounds[i][1] > 0]
            + [101 + 2 * i for i in active if bounds[i][0] < 0]
        )
        assert instance.constraint_hints.sos1_constraints == [
            v1.Sos1(
                binary_constraint_id=23, big_m_constraint_ids=links, variables=active
            )
        ]
        assert [row.id for row in instance.constraints] == [23] + links
    else:
        assert not instance.constraint_hints.sos1_constraints
        assert not list(instance.constraints)

    # Exhaust every selector assignment to test projection onto the original variables.
    # Use exactly representable, in-domain values; no finite-ATol equivalence is asserted.
    grids = [
        sorted({lower, upper} | ({0.0} if lower <= 0 <= upper else set()))
        for lower, upper in bounds
    ]
    for values in product(*grids):
        expected_objective = sum(c * x for c, x in zip(costs, values))
        feasible = False
        for selected in product([0, 1], repeat=len(selectors)):
            state = dict(enumerate(values)) | dict(zip(selectors, selected))
            solution = instance.evaluate(state)
            assert solution.objective == pytest.approx(expected_objective)
            feasible |= solution.feasible
        assert feasible == (sum(x != 0 for x in values) <= 1)


def test_unsupported_objective_retains_receiver_error():
    with pytest.raises(ommx.BridgeError) as error:
        modeling.compile_absolute_objective([1.0, -2.0])
    assert isinstance(error.value.__cause__, ommx.BridgeError)
    assert error.value.__cause__.__cause__ is not None


@pytest.mark.parametrize("costs", [[], [float("nan")], [float("inf")]])
def test_invalid_costs_are_rejected(costs: list[float]):
    with pytest.raises(ValueError):
        modeling.compile_one_hot(costs)


@pytest.mark.parametrize("bounds", [[], [(2.0, 1.0)], [(0.0, float("inf"))]])
def test_invalid_bounds_are_rejected(bounds: list[tuple[float, float]]):
    with pytest.raises(ValueError):
        modeling.compile_sos1([1.0], bounds)
