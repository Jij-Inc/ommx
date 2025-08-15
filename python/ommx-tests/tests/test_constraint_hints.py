"""Tests for ConstraintHints validation functionality."""

import pytest
import ommx._ommx_rust as _ommx_rust


def test_constraint_hints_constraint_id_validation():
    """Test that constraint hints validate referenced constraint IDs exist."""
    decision_variables = {1: _ommx_rust.DecisionVariable.binary(1)}
    objective = _ommx_rust.Function.from_scalar(1.0)
    constraints = {}  # Empty constraints

    # Create constraint hints that reference non-existent constraint ID
    one_hot = _ommx_rust.OneHot(id=999, variables=[1])  # ID 999 doesn't exist
    constraint_hints = _ommx_rust.ConstraintHints(
        one_hot_constraints=[one_hot], sos1_constraints=[]
    )

    # This should fail with validation error
    with pytest.raises(RuntimeError, match="Undefined constraint ID is used"):
        _ommx_rust.Instance.from_components(
            sense=_ommx_rust.Sense.Minimize,
            objective=objective,
            decision_variables=decision_variables,
            constraints=constraints,
            constraint_hints=constraint_hints,
        )


def test_constraint_hints_variable_id_validation():
    """Test that constraint hints validate referenced variable IDs exist."""
    decision_variables = {1: _ommx_rust.DecisionVariable.binary(1)}
    objective = _ommx_rust.Function.from_scalar(1.0)

    # Create a constraint
    constraint_func = _ommx_rust.Function.from_scalar(1.0)
    constraint = _ommx_rust.Constraint(
        id=1, function=constraint_func, equality=_ommx_rust.Equality.EqualToZero
    )
    constraints = {1: constraint}

    # Create constraint hints that reference non-existent variable ID
    one_hot = _ommx_rust.OneHot(id=1, variables=[1, 999])  # Variable 999 doesn't exist
    constraint_hints = _ommx_rust.ConstraintHints(
        one_hot_constraints=[one_hot], sos1_constraints=[]
    )

    # This should fail with validation error
    with pytest.raises(RuntimeError, match="Undefined variable ID is used"):
        _ommx_rust.Instance.from_components(
            sense=_ommx_rust.Sense.Minimize,
            objective=objective,
            decision_variables=decision_variables,
            constraints=constraints,
            constraint_hints=constraint_hints,
        )


def test_sos1_constraint_id_validation():
    """Test that SOS1 hints validate referenced constraint IDs exist."""
    decision_variables = {
        1: _ommx_rust.DecisionVariable.binary(1),
        2: _ommx_rust.DecisionVariable.binary(2),
    }
    objective = _ommx_rust.Function.from_scalar(1.0)
    constraints = {}  # Empty constraints

    # Create SOS1 hint that references non-existent constraint IDs
    sos1 = _ommx_rust.Sos1(
        binary_constraint_id=999,  # Doesn't exist
        big_m_constraint_ids=[1000, 1001],  # Don't exist
        variables=[1, 2],
    )
    constraint_hints = _ommx_rust.ConstraintHints(
        one_hot_constraints=[], sos1_constraints=[sos1]
    )

    # This should fail with validation error
    with pytest.raises(RuntimeError, match="Undefined constraint ID is used"):
        _ommx_rust.Instance.from_components(
            sense=_ommx_rust.Sense.Minimize,
            objective=objective,
            decision_variables=decision_variables,
            constraints=constraints,
            constraint_hints=constraint_hints,
        )


def test_sos1_variable_id_validation():
    """Test that SOS1 hints validate referenced variable IDs exist."""
    decision_variables = {1: _ommx_rust.DecisionVariable.binary(1)}
    objective = _ommx_rust.Function.from_scalar(1.0)

    # Create required constraints
    constraint_func = _ommx_rust.Function.from_scalar(1.0)
    binary_constraint = _ommx_rust.Constraint(
        id=1, function=constraint_func, equality=_ommx_rust.Equality.EqualToZero
    )
    big_m_constraint1 = _ommx_rust.Constraint(
        id=2, function=constraint_func, equality=_ommx_rust.Equality.EqualToZero
    )
    big_m_constraint2 = _ommx_rust.Constraint(
        id=3, function=constraint_func, equality=_ommx_rust.Equality.EqualToZero
    )
    constraints = {1: binary_constraint, 2: big_m_constraint1, 3: big_m_constraint2}

    # Create SOS1 hint that references non-existent variable ID
    sos1 = _ommx_rust.Sos1(
        binary_constraint_id=1,
        big_m_constraint_ids=[2, 3],
        variables=[1, 999],  # Variable 999 doesn't exist
    )
    constraint_hints = _ommx_rust.ConstraintHints(
        one_hot_constraints=[], sos1_constraints=[sos1]
    )

    # This should fail with validation error
    with pytest.raises(RuntimeError, match="Undefined variable ID is used"):
        _ommx_rust.Instance.from_components(
            sense=_ommx_rust.Sense.Minimize,
            objective=objective,
            decision_variables=decision_variables,
            constraints=constraints,
            constraint_hints=constraint_hints,
        )


def test_valid_constraint_hints():
    """Test that valid constraint hints work correctly."""
    # Create decision variables
    decision_variables = {
        1: _ommx_rust.DecisionVariable.binary(1),
        2: _ommx_rust.DecisionVariable.binary(2),
        3: _ommx_rust.DecisionVariable.binary(3),
    }

    # Create objective
    objective = _ommx_rust.Function.from_scalar(1.0)

    # Create constraints that will be referenced by hints
    constraint_func = _ommx_rust.Function.from_linear(
        _ommx_rust.Linear(terms={1: 1.0, 2: 1.0, 3: 1.0}, constant=-1.0)
    )  # x1 + x2 + x3 - 1 = 0
    constraint = _ommx_rust.Constraint(
        id=1, function=constraint_func, equality=_ommx_rust.Equality.EqualToZero
    )
    constraints = {1: constraint}

    # Create valid constraint hints
    one_hot = _ommx_rust.OneHot(id=1, variables=[1, 2, 3])  # References constraint ID 1
    constraint_hints = _ommx_rust.ConstraintHints(
        one_hot_constraints=[one_hot], sos1_constraints=[]
    )

    # This should succeed
    instance = _ommx_rust.Instance.from_components(
        sense=_ommx_rust.Sense.Minimize,
        objective=objective,
        decision_variables=decision_variables,
        constraints=constraints,
        constraint_hints=constraint_hints,
    )

    # Verify constraint hints are properly stored
    retrieved_hints = instance.constraint_hints
    assert len(retrieved_hints.one_hot_constraints) == 1
    assert len(retrieved_hints.sos1_constraints) == 0

    retrieved_one_hot = retrieved_hints.one_hot_constraints[0]
    assert retrieved_one_hot.id == 1
    assert retrieved_one_hot.variables == [1, 2, 3]


def test_sos1_variable_constraint_mapping():
    """Test that Sos1 handles variable-constraint mapping correctly."""
    # Test case 1: More variables than big-M constraints (excess variables get None)
    sos1 = _ommx_rust.Sos1(
        binary_constraint_id=1,
        big_m_constraint_ids=[2, 3],  # 2 constraints
        variables=[1, 2, 3],  # 3 variables - third one gets None
    )
    assert sos1.binary_constraint_id == 1
    assert len(sos1.variables) == 3
    assert len(sos1.big_m_constraint_ids) == 2  # Only 2 non-None constraints

    # Test case 2: Fewer big-M constraints than variables (some variables get None)
    sos1 = _ommx_rust.Sos1(
        binary_constraint_id=1,
        big_m_constraint_ids=[],  # 0 constraints
        variables=[1, 2],  # 2 variables - both get None constraint
    )
    assert sos1.binary_constraint_id == 1
    assert len(sos1.variables) == 2
    assert len(sos1.big_m_constraint_ids) == 0

    # Test case 3: Empty case
    sos1_empty = _ommx_rust.Sos1(
        binary_constraint_id=1,
        big_m_constraint_ids=[],  # 0 constraints
        variables=[],  # 0 variables
    )
    assert sos1_empty.binary_constraint_id == 1
    assert sos1_empty.variables == []
    assert sos1_empty.big_m_constraint_ids == []
