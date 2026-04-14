"""Tests for indicator constraint support in PySCIPOpt adapter."""

from ommx.v1 import Instance, DecisionVariable, Constraint
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
        constraints=[],
        indicator_constraints=[ic],
        sense=Instance.MAXIMIZE,
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
        constraints=[b >= 1],  # Force b = 1
        indicator_constraints=[ic],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # b must be 1, so x <= 5
    assert abs(solution.objective - 5.0) < 1e-6


def test_indicator_constraint_eq():
    """Test indicator constraint with equality: b=1 → x == 3."""
    b = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=0, upper=10)

    # b = 1 → x == 3 (i.e., x - 3 == 0)
    ic = (x == 3).with_indicator(b).set_id(100)

    instance = Instance.from_components(
        decision_variables=[b, x],
        objective=x,
        constraints=[b >= 1],  # Force b = 1
        indicator_constraints=[ic],
        sense=Instance.MAXIMIZE,
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
    ic1 = (x <= 50).with_indicator(b1).set_id(10)
    # b2 = 1 → x <= 30
    ic2 = (x <= 30).with_indicator(b2).set_id(11)

    instance = Instance.from_components(
        decision_variables=[b1, b2, x],
        objective=x,
        constraints=[
            (b1 + b2 >= 1).set_id(0),  # At least one indicator must be on
        ],
        indicator_constraints=[ic1, ic2],
        sense=Instance.MAXIMIZE,
    )

    solution = OMMXPySCIPOptAdapter.solve(instance)

    # Optimal: b1=1, b2=0, x=50 (only the weaker constraint is enforced)
    assert abs(solution.objective - 50.0) < 1e-6
