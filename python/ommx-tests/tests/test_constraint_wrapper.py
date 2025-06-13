"""Test Constraint PyO3 wrapper functionality."""

import ommx._ommx_rust as rust


def test_constraint_creation():
    """Test basic Constraint creation with equal to zero."""
    # Create a simple linear function: x1 + 2
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)

    # Create constraint: x1 + 2 = 0
    constraint = rust.Constraint(
        1, function, rust.Equality.EqualToZero, "test_constraint"
    )

    assert constraint.id == 1
    assert constraint.equality == rust.Equality.EqualToZero
    assert constraint.name == "test_constraint"
    assert constraint.subscripts == []


def test_constraint_less_than_or_equal():
    """Test Constraint creation with less than or equal to zero."""
    # Create a simple linear function: 2*x2 - 5
    linear = rust.Linear({2: 2.0}, -5.0)
    function = rust.Function.from_linear(linear)

    # Create constraint: 2*x2 - 5 <= 0
    constraint = rust.Constraint(
        2, function, rust.Equality.LessThanOrEqualToZero, "leq_constraint"
    )

    assert constraint.id == 2
    assert constraint.equality == rust.Equality.LessThanOrEqualToZero
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
        equality=rust.Equality.EqualToZero,
        name="quadratic_constraint",
        subscripts=[1, 2],
    )

    assert constraint.id == 3
    assert constraint.equality == rust.Equality.EqualToZero
    assert constraint.name == "quadratic_constraint"
    assert constraint.subscripts == [1, 2]


def test_constraint_function_access():
    """Test accessing function from constraint."""
    # Create a linear function
    linear = rust.Linear({1: 3.0, 2: -1.0}, 10.0)
    function = rust.Function.from_linear(linear)

    constraint = rust.Constraint(5, function, rust.Equality.EqualToZero, "access_test")

    # Access the function from constraint
    retrieved_function = constraint.function
    assert isinstance(retrieved_function, rust.Function)


def test_constraint_repr():
    """Test constraint string representation."""
    linear = rust.Linear.constant(5.0)
    function = rust.Function.from_linear(linear)

    # Test EqualToZero representation
    constraint1 = rust.Constraint(1, function, rust.Equality.EqualToZero, "eq_test")
    repr_str1 = repr(constraint1)
    assert "Constraint(5 == 0)" == repr_str1

    # Test LessThanOrEqualToZero representation
    constraint2 = rust.Constraint(
        2, function, rust.Equality.LessThanOrEqualToZero, "leq_test"
    )
    repr_str2 = repr(constraint2)
    assert "Constraint(5 <= 0)" == repr_str2


def test_constraint_empty_name():
    """Test constraint with no name."""
    linear = rust.Linear.single_term(1, 1.0)
    function = rust.Function.from_linear(linear)

    # Create constraint without name
    constraint = rust.Constraint(1, function, rust.Equality.EqualToZero, None)

    assert constraint.id == 1
    assert constraint.name == ""  # Should return empty string for None name
