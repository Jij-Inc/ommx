"""Test RemovedConstraint PyO3 wrapper functionality."""

import pytest
import ommx._ommx_rust as rust


def test_removed_constraint_creation():
    """Test basic RemovedConstraint creation."""
    # Create a constraint first
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint.equal_to_zero(1, function, "test_constraint")

    # Create removed constraint
    removed_constraint = rust.RemovedConstraint(
        constraint=constraint,
        removed_reason="Infeasible",
        removed_reason_parameters={"method": "penalty", "weight": "1000"},
    )

    assert removed_constraint.id == 1
    assert removed_constraint.removed_reason == "Infeasible"
    assert removed_constraint.removed_reason_parameters == {
        "method": "penalty",
        "weight": "1000",
    }
    assert removed_constraint.name == "test_constraint"


def test_removed_constraint_no_parameters():
    """Test RemovedConstraint creation without parameters."""
    # Create a constraint
    linear = rust.Linear({2: 2.0}, -5.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint.less_than_or_equal_to_zero(
        2, function, "leq_constraint"
    )

    # Create removed constraint without parameters
    removed_constraint = rust.RemovedConstraint(
        constraint=constraint, removed_reason="Redundant"
    )

    assert removed_constraint.id == 2
    assert removed_constraint.removed_reason == "Redundant"
    assert removed_constraint.removed_reason_parameters == {}
    assert removed_constraint.name == "leq_constraint"


def test_removed_constraint_access_original_constraint():
    """Test accessing the original constraint from RemovedConstraint."""
    # Create a polynomial constraint
    terms = {(1, 1): 1.0, (2,): 1.0}  # x1^2 + x2
    polynomial = rust.Polynomial(terms)
    function = rust.Function.from_polynomial(polynomial)
    original_constraint = rust.Constraint.equal_to_zero(3, function, "original")

    # Create removed constraint
    removed_constraint = rust.RemovedConstraint(
        constraint=original_constraint,
        removed_reason="Timeout",
        removed_reason_parameters={"timeout_seconds": "300"},
    )

    # Access the original constraint
    retrieved_constraint = removed_constraint.constraint
    assert isinstance(retrieved_constraint, rust.Constraint)
    assert retrieved_constraint.id == 3
    assert retrieved_constraint.name == "original"
    assert retrieved_constraint.equality == 1  # EqualToZero


def test_removed_constraint_repr():
    """Test RemovedConstraint string representation."""
    linear = rust.Linear.constant(5.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint.equal_to_zero(1, function, "repr_test")

    removed_constraint = rust.RemovedConstraint(
        constraint=constraint, removed_reason="Test reason"
    )

    repr_str = repr(removed_constraint)
    expected = 'RemovedConstraint(id=1, reason="Test reason", name="repr_test")'
    assert repr_str == expected


def test_removed_constraint_empty_name():
    """Test RemovedConstraint with constraint that has no name."""
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint.equal_to_zero(1, function, None)  # No name

    removed_constraint = rust.RemovedConstraint(
        constraint=constraint, removed_reason="No name test"
    )

    assert removed_constraint.name == ""  # Should return empty string
    repr_str = repr(removed_constraint)
    expected = 'RemovedConstraint(id=1, reason="No name test", name="")'
    assert repr_str == expected


def test_removed_constraint_complex_parameters():
    """Test RemovedConstraint with complex parameter structure."""
    linear = rust.Linear({1: 1.0, 2: -1.0}, 0.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint.less_than_or_equal_to_zero(5, function, "complex_test")

    complex_params = {
        "solver": "highs",
        "method": "dual_simplex",
        "tolerance": "1e-9",
        "max_iterations": "10000",
    }

    removed_constraint = rust.RemovedConstraint(
        constraint=constraint,
        removed_reason="Solver specific",
        removed_reason_parameters=complex_params,
    )

    assert removed_constraint.removed_reason_parameters == complex_params
    assert len(removed_constraint.removed_reason_parameters) == 4


def test_removed_constraint_getter_properties():
    """Test all getter properties of RemovedConstraint."""
    linear = rust.Linear({3: 0.5}, 2.0)
    function = rust.Function.from_linear(linear)
    constraint = rust.Constraint(
        id=10,
        function=function,
        equality=2,  # LessThanOrEqualToZero
        name="getter_test",
        subscripts=[1, 2, 3],
    )

    removed_constraint = rust.RemovedConstraint(
        constraint=constraint,
        removed_reason="Property test",
        removed_reason_parameters={"key": "value"},
    )

    # Test all properties
    assert removed_constraint.id == 10
    assert removed_constraint.name == "getter_test"
    assert removed_constraint.removed_reason == "Property test"
    assert removed_constraint.removed_reason_parameters == {"key": "value"}

    # Test that the constraint property returns the right type and values
    retrieved_constraint = removed_constraint.constraint
    assert retrieved_constraint.id == 10
    assert retrieved_constraint.name == "getter_test"
    assert retrieved_constraint.equality == 2
    assert retrieved_constraint.subscripts == [1, 2, 3]
