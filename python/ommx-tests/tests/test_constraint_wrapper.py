"""Test Constraint PyO3 wrapper functionality."""

import pytest
import ommx._ommx_rust as rust


def test_constraint_creation():
    """Test basic Constraint creation with equal to zero."""
    # Create a simple linear function: x1 + 2
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)

    # Create constraint: x1 + 2 = 0
    constraint = rust.Constraint(1, function, 1, "test_constraint")  # 1 = EqualToZero

    assert constraint.id == 1
    assert constraint.equality == 1  # EqualToZero
    assert constraint.name == "test_constraint"
    assert constraint.subscripts == []


def test_constraint_less_than_or_equal():
    """Test Constraint creation with less than or equal to zero."""
    # Create a simple linear function: 2*x2 - 5
    linear = rust.Linear({2: 2.0}, -5.0)
    function = rust.Function.from_linear(linear)

    # Create constraint: 2*x2 - 5 <= 0
    constraint = rust.Constraint(2, function, 2, "leq_constraint")  # 2 = LessThanOrEqualToZero

    assert constraint.id == 2
    assert constraint.equality == 2  # LessThanOrEqualToZero
    assert constraint.name == "leq_constraint"
    assert constraint.subscripts == []


def test_constraint_direct_constructor():
    """Test Constraint creation using direct constructor."""
    # Create quadratic function: x1^2 + x2
    terms = {(1, 1): 1.0, (2,): 1.0}  # x1^2 + x2
    polynomial = rust.Polynomial(terms)
    function = rust.Function.from_polynomial(polynomial)

    # Create constraint using direct constructor
    constraint = rust.Constraint(
        id=3,
        function=function,
        equality=1,  # EqualToZero
        name="quadratic_constraint",
        subscripts=[1, 2],
    )

    assert constraint.id == 3
    assert constraint.equality == 1
    assert constraint.name == "quadratic_constraint"
    assert constraint.subscripts == [1, 2]


def test_constraint_function_access():
    """Test accessing function from constraint."""
    # Create a linear function
    linear = rust.Linear({1: 3.0, 2: -1.0}, 10.0)
    function = rust.Function.from_linear(linear)

    constraint = rust.Constraint(5, function, 1, "access_test")  # 1 = EqualToZero

    # Access the function from constraint
    retrieved_function = constraint.function
    assert isinstance(retrieved_function, rust.Function)


def test_constraint_repr():
    """Test constraint string representation."""
    linear = rust.Linear.constant(5.0)
    function = rust.Function.from_linear(linear)

    # Test EqualToZero representation
    constraint1 = rust.Constraint(1, function, 1, "eq_test")  # 1 = EqualToZero
    repr_str1 = repr(constraint1)
    assert 'Constraint(id=1, equality=EqualToZero, name="eq_test")' == repr_str1

    # Test LessThanOrEqualToZero representation
    constraint2 = rust.Constraint(2, function, 2, "leq_test")  # 2 = LessThanOrEqualToZero
    repr_str2 = repr(constraint2)
    assert (
        'Constraint(id=2, equality=LessThanOrEqualToZero, name="leq_test")' == repr_str2
    )


def test_constraint_invalid_equality():
    """Test that invalid equality values raise errors."""
    linear = rust.Linear.constant(1.0)
    function = rust.Function.from_linear(linear)

    with pytest.raises(Exception):  # Should raise error for invalid equality
        rust.Constraint(1, function, 999, "invalid", None)


def test_constraint_empty_name():
    """Test constraint with no name."""
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)

    # Create constraint without name
    constraint = rust.Constraint(1, function, 1, None)  # 1 = EqualToZero

    assert constraint.id == 1
    assert constraint.name == ""  # Should return empty string for None name
