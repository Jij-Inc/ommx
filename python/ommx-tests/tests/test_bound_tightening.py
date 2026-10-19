from ommx import Bound, DecisionVariable, Function, Instance, Sense
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


@pytest.mark.parametrize("reverse", [False, True])
@pytest.mark.parametrize(
    ("terms", "expected"),
    [
        ([(1, -5), (1, -4.9375)], Bound(-10, 4.9375)),
        ([(-1, -5), (-1, -4.9375)], Bound(-4.9375, 10)),
        ([(1, 0), (-1, 0.125)], Bound(0, 0.125)),
        ([(1, 0), (-1, 0.25)], Bound(0, 0.25)),
        ([(1, -2), (-1, 4)], None),
    ],
)
def test_candidate_aggregation_is_independent_of_constraint_ids(
    reverse: bool, terms: list[tuple[float, float]], expected: Bound | None
) -> None:
    x = DecisionVariable.continuous(0, lower=-10, upper=10)
    rows = [a * x + c <= 0 for a, c in terms]
    if reverse:
        rows.reverse()
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x],
        constraints=dict(enumerate(rows)),
    )
    if expected is None:
        before = instance.to_v2_bytes()
        with pytest.raises(RuntimeError, match="incompatible bounds"):
            instance.tighten_bounds_simultaneously_once(atol=0.125)
        assert instance.to_v2_bytes() == before
    else:
        assert instance.tighten_bounds_simultaneously_once(atol=0.125) == {0: expected}
        assert instance.get_decision_variable_by_id(0).bound == expected


@pytest.mark.parametrize("selected", [False, True])
@pytest.mark.parametrize("num_terms", [32, 33])
def test_default_term_limit_and_explicit_override(
    selected: bool, num_terms: int
) -> None:
    variables = [
        DecisionVariable.integer(i, lower=0, upper=10) for i in range(num_terms)
    ]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=variables,
        constraints={0: sum(variables, Function(0)) <= 2},
    )
    if selected:
        changed = instance.tighten_bounds_simultaneously_once_using_constraints({0})
    else:
        changed = instance.tighten_bounds_simultaneously_once()
    assert changed == (
        {i: Bound(0, 2) for i in range(num_terms)} if num_terms == 32 else {}
    )
    if num_terms == 33:
        if selected:
            changed = instance.tighten_bounds_simultaneously_once_using_constraints(
                {0}, max_terms=33
            )
        else:
            changed = instance.tighten_bounds_simultaneously_once(max_terms=33)
        assert changed == {i: Bound(0, 2) for i in range(num_terms)}


@pytest.mark.parametrize("selected", [False, True])
def test_zero_and_negative_term_limits(selected: bool) -> None:
    instance = bound_tightening_instance()
    before = instance.to_v2_bytes()
    if selected:
        assert (
            instance.tighten_bounds_simultaneously_once_using_constraints(
                {0, 1}, max_terms=0
            )
            == {}
        )
        with pytest.raises(OverflowError):
            instance.tighten_bounds_simultaneously_once_using_constraints(
                {0, 1}, max_terms=-1
            )
    else:
        assert instance.tighten_bounds_simultaneously_once(max_terms=0) == {}
        with pytest.raises(OverflowError):
            instance.tighten_bounds_simultaneously_once(max_terms=-1)
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("selected", [False, True])
def test_unbounded_target_can_be_tightened_from_a_scaled_row(selected: bool) -> None:
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x],
        constraints={0: 2 * x <= 6},
    )
    if selected:
        changed = instance.tighten_bounds_simultaneously_once_using_constraints(
            {0}, atol=0.125
        )
    else:
        changed = instance.tighten_bounds_simultaneously_once(atol=0.125)
    # The row permits x <= 3.0625, including the bound's own tolerance.
    assert changed == {0: Bound(float("-inf"), 2.9375)}
    assert instance.get_decision_variable_by_id(0).bound == changed[0]


def test_nonfinite_candidates_do_not_discard_a_finite_candidate_in_the_same_row() -> (
    None
):
    x = DecisionVariable.continuous(0)
    y = DecisionVariable.continuous(1, lower=1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, y],
        constraints={0: x + y == 6},
    )
    assert instance.tighten_bounds_simultaneously_once(atol=0.125) == {
        0: Bound(float("-inf"), 5.125)
    }
    assert instance.get_decision_variable_by_id(1).bound == Bound(1, float("inf"))


def test_cancellation_uses_algebraic_bound_without_point_boundary_search() -> None:
    x = DecisionVariable.continuous(0)
    y = DecisionVariable.continuous(1, lower=1e16, upper=1e16)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x, y],
        constraints={0: x - y + 1e16 <= 0},
    )
    assert instance.tighten_bounds_simultaneously_once(atol=0.125) == {
        0: Bound(float("-inf"), 0)
    }


@pytest.mark.parametrize(
    ("coefficient", "constant", "expected"),
    [
        (2, -0.5, Bound(0, 0)),
        (-2, 0.5, Bound(1, 1)),
        (2, -1.875, Bound(0, 1)),
        (-2, 0.125, Bound(0, 1)),
    ],
)
def test_binary_rounding_with_row_tolerance(
    coefficient: float, constant: float, expected: Bound
) -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x],
        constraints={0: coefficient * x + constant <= 0},
    )
    instance.tighten_bounds_simultaneously_once(atol=0.125)
    assert instance.get_decision_variable_by_id(0).bound == expected


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
