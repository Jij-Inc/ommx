from ommx import Bound, DecisionVariable, Instance, Sense
import pytest


def bound_tightening_instance() -> Instance:
    x = DecisionVariable.continuous(0, lower=-100, upper=100)
    z = DecisionVariable.binary(1)
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, z],
        constraints={0: x - 3 * z <= 0, 1: -x - 2 * z <= 0},
    )


def test_tighten_bounds_simultaneously_once_returns_changed_domains() -> None:
    instance = bound_tightening_instance()
    assert instance.tighten_bounds_simultaneously_once(atol=0.125) == {0: Bound(-2, 3)}
    assert instance.get_decision_variable_by_id(0).bound == Bound(-2, 3)
    assert instance.tighten_bounds_simultaneously_once(atol=0.125) == {}
    assert set(instance.constraints) == {0, 1}


@pytest.mark.parametrize(
    ("constraint_ids", "expected"),
    [
        ({0}, {0: Bound(-100, 3)}),
        ({1}, {0: Bound(-2, 100)}),
        ({0, 1}, {0: Bound(-2, 3)}),
        (set(), {}),
    ],
)
def test_selected_constraints_determine_bound_updates(
    constraint_ids: set[int], expected: dict[int, Bound]
) -> None:
    instance = bound_tightening_instance()
    changed = instance.tighten_bounds_simultaneously_once_using_constraints(
        constraint_ids, atol=0.125
    )
    assert changed == expected
    assert instance.get_decision_variable_by_id(0).bound == expected.get(
        0, Bound(-100, 100)
    )
    assert set(instance.constraints) == {0, 1}


def test_explicit_all_constraints_matches_all_constraints_api() -> None:
    selected = bound_tightening_instance()
    all_constraints = bound_tightening_instance()
    assert selected.tighten_bounds_simultaneously_once_using_constraints(
        {0, 1}, atol=0.125
    ) == all_constraints.tighten_bounds_simultaneously_once(atol=0.125)
    assert selected.to_v2_bytes() == all_constraints.to_v2_bytes()


@pytest.mark.parametrize("invalid", [1, 999])
def test_unknown_and_removed_selected_constraints_are_atomic(invalid: int) -> None:
    instance = bound_tightening_instance()
    instance.relax_constraint(1, "test")
    before = instance.to_v2_bytes()
    with pytest.raises(RuntimeError, match="is not active"):
        instance.tighten_bounds_simultaneously_once_using_constraints({0, invalid})
    assert instance.to_v2_bytes() == before


def test_tighten_bounds_simultaneously_once_is_atomic() -> None:
    x = DecisionVariable.integer(0, lower=0, upper=10)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x],
        constraints={0: x <= 2, 1: x >= 20},
    )
    before = instance.to_v2_bytes()
    with pytest.raises(RuntimeError, match="infeasible"):
        instance.tighten_bounds_simultaneously_once()
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("atol", [1.0, float("inf")])
def test_tighten_bounds_simultaneously_once_rejects_unsupported_tolerance(
    atol: float,
) -> None:
    instance = Instance.from_components(
        sense=Sense.Minimize, objective=0, decision_variables=[], constraints={}
    )
    with pytest.raises(RuntimeError, match="finite ATol smaller than one"):
        instance.tighten_bounds_simultaneously_once(atol=atol)
