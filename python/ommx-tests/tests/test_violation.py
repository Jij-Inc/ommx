"""Tests for constraint violation calculation methods."""

import pytest
from ommx.v1 import Instance, DecisionVariable


def test_evaluated_constraint_violation_equality():
    """Test violation calculation for equality constraints."""
    # Create instance with equality constraint: x = 2.5 evaluated at x=0
    # This gives f(x) = x - 2.5 = 0 - 2.5 = -2.5, violation = |-2.5| = 2.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = (x == 2.5).set_id(1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    # Evaluate at x=0, so constraint becomes 0 = 2.5, f(x) = -2.5
    solution = instance.evaluate({1: 0.0})
    evaluated_constraint = solution.constraints[0]

    # For equality constraint f(x) = 0, violation = |f(x)| = |-2.5| = 2.5
    assert evaluated_constraint.violation() == pytest.approx(2.5)


def test_evaluated_constraint_violation_inequality_violated():
    """Test violation calculation for violated inequality constraints."""
    # Create instance with inequality constraint: x <= 1.5 evaluated at x=3
    # This gives f(x) = x - 1.5 = 3 - 1.5 = 1.5, violation = max(0, 1.5) = 1.5
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = (x <= 1.5).set_id(1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    # Evaluate at x=3, so constraint becomes 3 <= 1.5, f(x) = 1.5
    solution = instance.evaluate({1: 3.0})
    evaluated_constraint = solution.constraints[0]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, 1.5) = 1.5
    assert evaluated_constraint.violation() == pytest.approx(1.5)


def test_evaluated_constraint_violation_inequality_satisfied():
    """Test violation calculation for satisfied inequality constraints."""
    # Create instance with inequality constraint: x <= 5 evaluated at x=2
    # This gives f(x) = x - 5 = 2 - 5 = -3, violation = max(0, -3) = 0
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)
    constraint = (x <= 5.0).set_id(1)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    # Evaluate at x=2, so constraint becomes 2 <= 5, f(x) = -3
    solution = instance.evaluate({1: 2.0})
    evaluated_constraint = solution.constraints[0]

    # For inequality constraint f(x) ≤ 0, violation = max(0, f(x)) = max(0, -3) = 0.0
    assert evaluated_constraint.violation() == pytest.approx(0.0)


def test_solution_total_violation_l1():
    """Test total L1 violation calculation."""
    x = DecisionVariable.continuous(id=1, lower=0, upper=10)

    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints=[
            (x == 2.5).set_id(
                1
            ),  # Equality: x = 2.5, evaluated at x=0 gives f(x) = -2.5
            (x <= 1.5).set_id(
                2
            ),  # Inequality: x <= 1.5, evaluated at x=3 gives f(x) = 1.5
        ],
        sense=Instance.MINIMIZE,
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
        constraints=[
            (x == 2.5).set_id(1),  # Equality: x = 2.5
            (x <= 1.5).set_id(2),  # Inequality: x <= 1.5
        ],
        sense=Instance.MINIMIZE,
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
        constraints=[
            (x == 2.0).set_id(
                1
            ),  # Violated equality: x = 2.0, evaluated at x=5 gives |5-2| = 3
            (x <= 10.0).set_id(
                2
            ),  # Satisfied inequality: x <= 10, evaluated at x=5 gives max(0, 5-10) = 0
        ],
        sense=Instance.MINIMIZE,
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
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    solution = instance.evaluate({1: 5.0})

    # No constraints means no violations
    assert solution.total_violation_l1() == pytest.approx(0.0)
    assert solution.total_violation_l2() == pytest.approx(0.0)
