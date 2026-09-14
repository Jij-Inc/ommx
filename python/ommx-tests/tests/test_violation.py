"""Tests for constraint violation calculation methods."""

import pytest
from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Sense,
    Solution,
    Sos1Constraint,
)


def test_evaluated_constraint_violation_equality():
    """Test violation calculation for equality constraints."""
    # Create instance with equality constraint: x = 2.5 evaluated at x=0
    # This gives f(x) = x - 2.5 = 0 - 2.5 = -2.5, violation = |-2.5| = 2.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x == 2.5

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=0, so constraint becomes 0 = 2.5, f(x) = -2.5
    solution = instance.evaluate({1: 0.0})
    evaluated_constraint = solution.constraints[1]

    # For equality constraint f(x) = 0, violation = |f(x)| = |-2.5| = 2.5
    assert evaluated_constraint.violation() == pytest.approx(2.5)


def test_evaluated_constraint_violation_inequality_violated():
    """Test violation calculation for violated inequality constraints."""
    # Create instance with inequality constraint: x <= 1.5 evaluated at x=3
    # This gives f(x) = x - 1.5 = 3 - 1.5 = 1.5, violation = max(0, 1.5) = 1.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x <= 1.5

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=3, so constraint becomes 3 <= 1.5, f(x) = 1.5
    solution = instance.evaluate({1: 3.0})
    evaluated_constraint = solution.constraints[1]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, 1.5) = 1.5
    assert evaluated_constraint.violation() == pytest.approx(1.5)


def test_evaluated_constraint_violation_inequality_satisfied():
    """Test violation calculation for satisfied inequality constraints."""
    # Create instance with inequality constraint: x <= 5 evaluated at x=2
    # This gives f(x) = x - 5 = 2 - 5 = -3, violation = max(0, -3) = 0
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = x <= 5.0

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={1: constraint},
        sense=Sense.Minimize,
    )

    # Evaluate at x=2, so constraint becomes 2 <= 5, f(x) = -3
    solution = instance.evaluate({1: 2.0})
    evaluated_constraint = solution.constraints[1]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, -3) = 0.0
    assert evaluated_constraint.violation() == pytest.approx(0.0)


def test_solution_total_violation_l1():
    """Test total L1 violation calculation."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            0: x == 2.5,  # Equality: x = 2.5, evaluated at x=0 gives f(x) = -2.5
            1: x <= 1.5,  # Inequality: x <= 1.5, evaluated at x=3 gives f(x) = 1.5
        },
        sense=Sense.Minimize,
    )

    # Use x=0 for equality (violation=2.5) but that would make inequality satisfied
    # So let's use x=5: equality violation = |5-2.5| = 2.5, inequality violation = max(0, 5-1.5) = 3.5
    solution = instance.evaluate({1: 5.0})

    # L1 = |5-2.5| + max(0, 5-1.5) = 2.5 + 3.5 = 6.0
    expected_l1 = 2.5 + 3.5
    assert solution.total_violation_l1() == pytest.approx(expected_l1)


def test_solution_total_violation_l2():
    """Test total L2 violation calculation."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            0: x == 2.5,  # Equality: x = 2.5
            1: x <= 1.5,  # Inequality: x <= 1.5
        },
        sense=Sense.Minimize,
    )

    # Evaluate at x=5: equality violation = 2.5, inequality violation = 3.5
    solution = instance.evaluate({1: 5.0})

    # L2 = (2.5)^2 + (3.5)^2 = 6.25 + 12.25 = 18.5
    expected_l2 = 2.5**2 + 3.5**2
    assert solution.total_violation_l2() == pytest.approx(expected_l2)


def test_solution_total_violation_with_satisfied_constraints():
    """Test total violation when some constraints are satisfied."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            0: x == 2.0,  # Violated equality: x = 2.0, evaluated at x=5 gives |5-2| = 3
            1: x
            <= 10.0,  # Satisfied inequality: x <= 10, evaluated at x=5 gives max(0, 5-10) = 0
        },
        sense=Sense.Minimize,
    )

    solution = instance.evaluate({1: 5.0})

    # L1 = |5-2.0| + max(0, 5-10) = 3.0 + 0.0 = 3.0
    assert solution.total_violation_l1() == pytest.approx(3.0)

    # L2 = (3.0)^2 + (0.0)^2 = 9.0 + 0.0 = 9.0
    assert solution.total_violation_l2() == pytest.approx(9.0)


def test_solution_total_violation_empty():
    """Test total violation with no constraints."""
    x = DecisionVariable.integer(id=1, lower=0, upper=10)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    solution = instance.evaluate({1: 5.0})

    # No constraints means no violations
    assert solution.total_violation_l1() == pytest.approx(0.0)
    assert solution.total_violation_l2() == pytest.approx(0.0)


@pytest.mark.parametrize("active", [False, True])
@pytest.mark.parametrize("equality", [False, True])
@pytest.mark.parametrize("removed", [False, True])
@pytest.mark.parametrize("value", [-2.5, 0.0, 1.5])
def test_total_violation_indicator(active, equality, removed, value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-10, upper=10)
    constraint = x == 0 if equality else x <= 0
    instance = Instance.from_components(
        decision_variables=[z, x],
        objective=0,
        constraints={},
        indicator_constraints={0: constraint.with_indicator(z)},
        sense=Sense.Minimize,
    )
    if removed:
        instance.relax_indicator_constraint(0, "test relaxation")
    solution = instance.evaluate({0: float(active), 1: value})
    violation = (abs(value) if equality else max(0, value)) if active else 0
    assert solution.total_violation_l1() == pytest.approx(violation)
    assert solution.total_violation_l2() == pytest.approx(violation**2)


@pytest.mark.parametrize("values", [[0, 0, 0], [0, 1, 0], [1, 1, 1], [0.5, 0.5, 0.5]])
def test_total_violation_one_hot_matches_lowering(values):
    xs = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = dict(enumerate(values))
    solution = instance.evaluate(state)
    row = instance.convert_one_hot_to_constraint(0)
    lowered = instance.evaluate(state)
    violation = lowered.constraints[row].violation()
    assert solution.total_violation_l1() == pytest.approx(violation)
    assert solution.total_violation_l2() == pytest.approx(violation**2)


@pytest.mark.parametrize(
    "values, expected_l1, expected_l2",
    [
        ([0.0, 0.0, 0.0, 0.0, 0.0], 0.0, 0.0),
        ([3.0, 0.0, 0.0, 0.0, 0.0], 0.0, 0.0),
        ([3.0, -5.0, 1.0, 4.0, -6.0], 4.0, 16.0),
        # Link violations 1, 2, 2, 3 plus cardinality violation 4.
        ([4.0, -7.0, 1.0, 6.0, -9.0], 12.0, 34.0),
        # Fresh selectors are zero at the inclusive tolerance boundary.
        # Instance.evaluate first canonicalizes the integer member to zero.
        ([0.25, -0.25, 0.0, 0.25, -0.25], 0.75, 0.1875),
        # The binary member is also canonicalized at the tolerance boundary.
        ([3.0, 0.0, 0.25, 0.0, 0.0], 0.0, 0.0),
        # Outside the tolerance it is reused verbatim, not converted to one.
        ([3.0, 0.0, 0.5, 0.0, 0.0], 0.5, 0.25),
        ([0.5, -0.5, 0.0, 0.5, -0.5], 3.0, 9.0),
    ],
)
def test_total_violation_sos1_matches_big_m_lowering(values, expected_l1, expected_l2):
    atol = 0.25
    xs = [
        DecisionVariable.continuous(0, lower=-2, upper=3),
        DecisionVariable.integer(1, lower=-5, upper=7),
        DecisionVariable.binary(2),
        DecisionVariable.continuous(3, lower=0, upper=4),
        DecisionVariable.continuous(4, lower=-6, upper=0),
    ]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    state = dict(enumerate(values))
    solution = instance.evaluate(state, atol=atol)
    sampled = instance.evaluate_samples({7: state}, atol=atol).get(7)
    restored = Solution.from_v2_bytes(solution.to_v2_bytes())

    rows = instance.convert_sos1_to_constraints(0)
    extended_state = state.copy()
    for variable in instance.decision_variables:
        if variable.id not in state:
            assert variable.name == "ommx.sos1_indicator"
            member_id = variable.subscripts[1]
            extended_state[variable.id] = float(abs(state[member_id]) > atol)
    lowered = instance.evaluate(extended_state, atol=atol)
    violations = [lowered.constraints[row].violation() for row in rows]
    assert sum(violations) == pytest.approx(expected_l1)
    assert sum(v**2 for v in violations) == pytest.approx(expected_l2)
    for result in [solution, sampled, restored]:
        assert result.total_violation_l1() == pytest.approx(expected_l1)
        assert result.total_violation_l2() == pytest.approx(expected_l2)


@pytest.mark.parametrize("values, violation", [([0.0, 0.0], 0.0), ([1.0, -2.0], 1.0)])
def test_total_violation_sos1_with_unbounded_members(values, violation):
    xs = [DecisionVariable.continuous(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=xs,
        objective=0,
        constraints={},
        sos1_constraints={0: Sos1Constraint(variables=xs)},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate(dict(enumerate(values)))
    assert solution.total_violation_l1() == pytest.approx(violation)
    assert solution.total_violation_l2() == pytest.approx(violation**2)
