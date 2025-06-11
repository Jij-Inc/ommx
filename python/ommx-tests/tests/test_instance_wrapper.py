"""Test Instance PyO3 wrapper functionality."""

import ommx._ommx_rust as rust


def test_instance_from_components():
    """Test Instance.from_components constructor."""
    # Create decision variables
    dv1 = rust.DecisionVariable.binary(1)
    dv2 = rust.DecisionVariable.continuous(2, lower=0.0, upper=10.0)
    decision_variables = {1: dv1, 2: dv2}
    
    # Create objective function: x1 + 2*x2
    linear = rust.Linear({1: 1.0, 2: 2.0}, 0.0)
    objective = rust.Function.from_linear(linear)
    
    # Create constraint: x1 + x2 <= 5
    constraint_linear = rust.Linear({1: 1.0, 2: 1.0}, -5.0)
    constraint_function = rust.Function.from_linear(constraint_linear)
    constraint = rust.Constraint(1, constraint_function, 2)  # 2 = LessThanOrEqualToZero
    constraints = {1: constraint}
    
    # Create instance with MINIMIZE sense
    instance = rust.Instance.from_components(
        sense=1,  # MINIMIZE
        objective=objective,
        decision_variables=decision_variables,
        constraints=constraints
    )
    
    assert isinstance(instance, rust.Instance)


def test_instance_getters():
    """Test Instance getter methods."""
    # Create simple instance
    dv = rust.DecisionVariable.binary(1)
    decision_variables = {1: dv}
    
    linear = rust.Linear.single_term(1, 1.0)
    objective = rust.Function.from_linear(linear)
    
    constraint_linear = rust.Linear.single_term(1, 1.0)
    constraint_function = rust.Function.from_linear(constraint_linear)
    constraint = rust.Constraint(1, constraint_function, 1)  # 1 = EqualToZero
    constraints = {1: constraint}
    
    instance = rust.Instance.from_components(
        sense=2,  # MAXIMIZE
        objective=objective,
        decision_variables=decision_variables,
        constraints=constraints
    )
    
    # Test get_sense
    sense = instance.get_sense()
    assert sense == 2  # MAXIMIZE
    
    # Test get_objective
    retrieved_objective = instance.get_objective()
    assert isinstance(retrieved_objective, rust.Function)
    
    # Test get_decision_variables
    retrieved_dvs = instance.get_decision_variables()
    assert isinstance(retrieved_dvs, dict)
    assert 1 in retrieved_dvs
    assert isinstance(retrieved_dvs[1], rust.DecisionVariable)
    
    # Test get_constraints
    retrieved_constraints = instance.get_constraints()
    assert isinstance(retrieved_constraints, dict)
    assert 1 in retrieved_constraints
    assert isinstance(retrieved_constraints[1], rust.Constraint)
    
    # Test get_removed_constraints (should be empty initially)
    removed_constraints = instance.get_removed_constraints()
    assert isinstance(removed_constraints, dict)
    assert len(removed_constraints) == 0


def test_instance_sense_validation():
    """Test that invalid sense values raise errors."""
    dv = rust.DecisionVariable.binary(1)
    decision_variables = {1: dv}
    
    linear = rust.Linear.single_term(1, 1.0)
    objective = rust.Function.from_linear(linear)
    
    constraints = {}
    
    # Test invalid sense value
    try:
        rust.Instance.from_components(
            sense=999,  # Invalid sense
            objective=objective,
            decision_variables=decision_variables,
            constraints=constraints
        )
        assert False, "Should have raised an exception for invalid sense"
    except Exception as e:
        assert "Invalid sense" in str(e)


def test_instance_serialization():
    """Test Instance serialization methods."""
    # Create simple instance
    dv = rust.DecisionVariable.binary(1)
    decision_variables = {1: dv}
    
    linear = rust.Linear.constant(5.0)
    objective = rust.Function.from_linear(linear)
    
    instance = rust.Instance.from_components(
        sense=1,  # MINIMIZE
        objective=objective,
        decision_variables=decision_variables,
        constraints={}
    )
    
    # Test to_bytes
    bytes_data = instance.to_bytes()
    assert isinstance(bytes_data, bytes)
    assert len(bytes_data) > 0
    
    # Test from_bytes
    instance2 = rust.Instance.from_bytes(bytes_data)
    assert isinstance(instance2, rust.Instance)
    
    # Verify deserialized instance has same properties
    assert instance2.get_sense() == 1
    assert len(instance2.get_decision_variables()) == 1
    assert len(instance2.get_constraints()) == 0