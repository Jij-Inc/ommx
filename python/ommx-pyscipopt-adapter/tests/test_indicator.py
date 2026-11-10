"""Tests for indicator constraint support in PySCIPOpt adapter."""

import pytest

from ommx import (
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    Sense,
)
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter


def test_indicator_constraint_le():
    """Test indicator constraint: b=1 → x <= 5."""
    b = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=0, upper=10)

    # b = 1 → x <= 5 (i.e., x - 5 <= 0)
    ic = (x <= 5).with_indicator(b)

    instance = Instance.from_components(
        decision_variables=[b, x],
        objective=x,
        active_constraints={},
        active_indicator_constraints={0: ic},
        sense=Sense.Maximize,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # Since b can be 0, x can go up to 10
    assert abs(solution.objective - 10.0) < 1e-6


def test_indicator_constraint_forced_on():
    """Test indicator constraint where indicator must be 1."""
    b = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=0, upper=10)

    # b = 1 → x <= 5
    ic = (x <= 5).with_indicator(b)

    instance = Instance.from_components(
        decision_variables=[b, x],
        objective=x,
        active_constraints={0: b >= 1},  # Force b = 1
        active_indicator_constraints={1: ic},
        sense=Sense.Maximize,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # b must be 1, so x <= 5
    assert abs(solution.objective - 5.0) < 1e-6


def test_indicator_constraint_eq():
    """Test indicator constraint with equality: b=1 → x == 3."""
    b = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=0, upper=10)

    # b = 1 → x == 3 (i.e., x - 3 == 0)
    ic = (x == 3).with_indicator(b)

    instance = Instance.from_components(
        decision_variables=[b, x],
        objective=x,
        active_constraints={0: b >= 1},  # Force b = 1
        active_indicator_constraints={1: ic},
        sense=Sense.Maximize,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # b must be 1, so x == 3
    assert abs(solution.objective - 3.0) < 1e-6


def test_indicator_constraint_multiple():
    """Test multiple indicator constraints."""
    b1 = DecisionVariable.binary(0)
    b2 = DecisionVariable.binary(1)
    x = DecisionVariable.continuous(2, lower=0, upper=100)

    # b1 = 1 → x <= 50
    ic1 = (x <= 50).with_indicator(b1)
    # b2 = 1 → x <= 30
    ic2 = (x <= 30).with_indicator(b2)

    instance = Instance.from_components(
        decision_variables=[b1, b2, x],
        objective=x,
        # At least one indicator must be on
        active_constraints={0: b1 + b2 >= 1},
        active_indicator_constraints={10: ic1, 11: ic2},
        sense=Sense.Maximize,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # Optimal: b1=1, b2=0, x=50 (only the weaker constraint is enforced)
    assert abs(solution.objective - 50.0) < 1e-6


@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("indicator_on", [False, True])
@pytest.mark.parametrize("sense", [Sense.Minimize, Sense.Maximize])
def test_both_activation_values(zero, equality, indicator_on, sense):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=0, upper=10)
    value = int(not zero) if indicator_on else int(zero)
    instance = Instance.from_components(
        sense=sense,
        objective=x,
        decision_variables=[z, x],
        active_constraints={0: z == value},
        active_indicator_constraints={
            1: IndicatorConstraint(
                indicator_variable=z,
                function=x - 3,
                equality=equality,
                activate_on_zero=zero,
            )
        },
    )
    assert OMMXPySCIPOptAdapter.INPUT_CLASS.contains(instance)
    solution = OMMXPySCIPOptAdapter.solve_without_preparation(instance)
    expected = (
        3
        if indicator_on
        and (equality == Equality.EqualToZero or sense == Sense.Maximize)
        else (0 if sense == Sense.Minimize else 10)
    )
    assert solution.objective == pytest.approx(expected)
    assert solution.feasible


@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("residual", [-1.0, 0.0, 1.0])
def test_constant_body_uses_indicator_off_value(zero, equality, residual):
    z = DecisionVariable.binary(0)
    body_feasible = residual == 0 if equality == Equality.EqualToZero else residual <= 0
    # Prefer the ON trigger: a feasible constant permits it, an infeasible one forbids it.
    instance = Instance.from_components(
        sense=Sense.Minimize if zero else Sense.Maximize,
        objective=z,
        decision_variables=[z],
        active_constraints={},
        active_indicator_constraints={
            0: IndicatorConstraint(
                indicator_variable=z,
                function=Function.from_scalar(residual),
                equality=equality,
                activate_on_zero=zero,
            )
        },
    )
    solution = OMMXPySCIPOptAdapter.solve_without_preparation(instance)
    assert solution.objective == int(not zero if body_feasible else zero)
    assert solution.feasible
