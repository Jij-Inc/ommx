"""Test decision_variable_analysis API implementation."""

import pytest
from ommx.v1 import DecisionVariable, Instance


def test_decision_variable_analysis_basic():
    """Test basic decision_variable_analysis functionality."""
    # Create binary variables
    x = [DecisionVariable.binary(i, name="x") for i in range(3)]

    # Create instance with objective and constraints
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[(x[1] + x[2] == 1).set_id(0)],
        sense=Instance.MAXIMIZE,
    )

    # Test decision_variable_analysis method
    analysis = instance.decision_variable_analysis()

    # Test basic functionality
    used_ids = analysis.used_decision_variable_ids()
    assert used_ids == {0, 1, 2}

    # Test used_in_objective
    objective_vars = analysis.used_in_objective()
    assert objective_vars == {0, 1}

    # Test used_in_constraints
    constraint_vars = analysis.used_in_constraints()
    assert 0 in constraint_vars
    assert constraint_vars[0] == {1, 2}

    # Test used_binary returns Bound objects
    binary_vars = analysis.used_binary()
    assert len(binary_vars) == 3
    for var_id, bound in binary_vars.items():
        assert var_id in {0, 1, 2}
        assert bound.lower == 0.0
        assert bound.upper == 1.0


def test_decision_variable_analysis_mixed_types():
    """Test decision_variable_analysis with mixed variable types."""
    # Create mixed variables
    x_bin = DecisionVariable.binary(0, name="x_bin")
    x_int = DecisionVariable.integer(1, lower=0, upper=5, name="x_int")
    x_cont = DecisionVariable.continuous(2, lower=0, upper=10, name="x_cont")

    # Create instance
    instance = Instance.from_components(
        decision_variables=[x_bin, x_int, x_cont],
        objective=x_bin + x_int + x_cont,
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    # Get analysis
    analysis = instance.decision_variable_analysis()

    # Test binary variables
    binary_vars = analysis.used_binary()
    assert 0 in binary_vars
    assert binary_vars[0].lower == 0.0
    assert binary_vars[0].upper == 1.0

    # Test integer variables
    integer_vars = analysis.used_integer()
    assert 1 in integer_vars
    assert integer_vars[1].lower == 0.0
    assert integer_vars[1].upper == 5.0

    # Test continuous variables
    continuous_vars = analysis.used_continuous()
    assert 2 in continuous_vars
    assert continuous_vars[2].lower == 0.0
    assert continuous_vars[2].upper == 10.0


def test_bound_wrapper_functionality():
    """Test that Bound wrapper works correctly."""
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[],
        sense=Instance.MAXIMIZE,
    )

    analysis = instance.decision_variable_analysis()
    binary_vars = analysis.used_binary()

    # Test a specific bound object
    bound = binary_vars[0]

    # Test bound methods
    assert bound.lower == 0.0
    assert bound.upper == 1.0
    assert bound.width() == 1.0
    assert bound.is_finite() == True
    assert bound.contains(0.5, 0.001) == True
    assert bound.contains(-0.1, 0.001) == False
    assert bound.nearest_to_zero() == 0.0

    # Test string representations
    assert "0" in str(bound)
    assert "1" in str(bound)
