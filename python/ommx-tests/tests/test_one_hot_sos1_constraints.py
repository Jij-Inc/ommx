"""Tests for OneHotConstraint and Sos1Constraint as first-class constraint types."""

import pytest
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint, Sos1Constraint


def test_one_hot_constraint_from_components():
    """Test creating an instance with OneHotConstraint via from_components."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3], id=10)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        one_hot_constraints=[oh],
        sense=Instance.MINIMIZE,
    )

    assert len(instance.one_hot_constraints) == 1
    assert instance.one_hot_constraints[0].id == 10
    assert instance.one_hot_constraints[0].variables == [1, 2, 3]


def test_sos1_constraint_from_components():
    """Test creating an instance with Sos1Constraint via from_components."""
    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3], id=20)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        sos1_constraints=[sos1],
        sense=Instance.MINIMIZE,
    )

    assert len(instance.sos1_constraints) == 1
    assert instance.sos1_constraints[0].id == 20
    assert instance.sos1_constraints[0].variables == [1, 2, 3]


def test_one_hot_variable_not_defined():
    """Test that OneHotConstraint with undefined variable ID fails."""
    x = [DecisionVariable.binary(1)]
    objective = x[0]

    oh = OneHotConstraint(variables=[1, 999], id=10)  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints=[],
            one_hot_constraints=[oh],
            sense=Instance.MINIMIZE,
        )


def test_sos1_variable_not_defined():
    """Test that Sos1Constraint with undefined variable ID fails."""
    x = [DecisionVariable.continuous(1, lower=0, upper=10)]
    objective = x[0]

    sos1 = Sos1Constraint(variables=[1, 999], id=20)  # 999 doesn't exist

    with pytest.raises(RuntimeError):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints=[],
            sos1_constraints=[sos1],
            sense=Instance.MINIMIZE,
        )


def test_one_hot_variable_not_binary():
    """Test that OneHotConstraint with non-binary variable fails."""
    x = [
        DecisionVariable.binary(1),
        DecisionVariable.continuous(2, lower=0, upper=1),  # not binary
    ]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2], id=10)

    with pytest.raises(RuntimeError, match="One-hot variable.*must be binary"):
        Instance.from_components(
            decision_variables=x,
            objective=objective,
            constraints=[],
            one_hot_constraints=[oh],
            sense=Instance.MINIMIZE,
        )


def test_serialize_not_yet_supported():
    """Serialization of OneHot/SOS1 constraints to v1 proto is not yet supported."""
    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        one_hot_constraints=[OneHotConstraint(variables=[1, 2, 3], id=10)],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(BaseException):
        instance.to_bytes()


def test_both_one_hot_and_sos1():
    """Test instance with both OneHot and SOS1 constraints."""
    x = [DecisionVariable.binary(i) for i in range(1, 6)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3], id=10)
    sos1 = Sos1Constraint(variables=[3, 4, 5], id=20)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        one_hot_constraints=[oh],
        sos1_constraints=[sos1],
        sense=Instance.MINIMIZE,
    )

    assert len(instance.one_hot_constraints) == 1
    assert len(instance.sos1_constraints) == 1


def test_evaluate_with_one_hot_feasible():
    """Test that evaluation with OneHot constraints checks feasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3], id=10)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        one_hot_constraints=[oh],
        sense=Instance.MINIMIZE,
    )

    # x1=0, x2=1, x3=0 → feasible (exactly one is 1)
    state = State({1: 0.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert solution.feasible


def test_evaluate_with_one_hot_infeasible():
    """Test that evaluation with OneHot constraints detects infeasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.binary(i) for i in range(1, 4)]
    objective = sum(x)

    oh = OneHotConstraint(variables=[1, 2, 3], id=10)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        one_hot_constraints=[oh],
        sense=Instance.MINIMIZE,
    )

    # x1=1, x2=1, x3=0 → infeasible (two are 1)
    state = State({1: 1.0, 2: 1.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible


def test_evaluate_with_sos1_feasible():
    """Test that evaluation with SOS1 constraints checks feasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3], id=20)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        sos1_constraints=[sos1],
        sense=Instance.MINIMIZE,
    )

    # x1=0, x2=5, x3=0 → feasible (at most one non-zero)
    state = State({1: 0.0, 2: 5.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert solution.feasible

    # All zeros → also feasible for SOS1
    state_zeros = State({1: 0.0, 2: 0.0, 3: 0.0})
    solution_zeros = instance.evaluate(state_zeros)
    assert solution_zeros.feasible


def test_evaluate_with_sos1_infeasible():
    """Test that evaluation with SOS1 constraints detects infeasibility."""
    from ommx.v1 import State

    x = [DecisionVariable.continuous(i, lower=0, upper=10) for i in range(1, 4)]
    objective = sum(x)

    sos1 = Sos1Constraint(variables=[1, 2, 3], id=20)

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[],
        sos1_constraints=[sos1],
        sense=Instance.MINIMIZE,
    )

    # x1=1, x2=2, x3=0 → infeasible (two non-zero)
    state = State({1: 1.0, 2: 2.0, 3: 0.0})
    solution = instance.evaluate(state)
    assert not solution.feasible
