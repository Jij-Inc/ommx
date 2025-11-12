"""Snapshot tests for print output of OMMX v1 classes."""

from ommx.v1 import (
    DecisionVariable,
    Linear,
    Function,
    Instance,
)


def test_linear_print_simple(snapshot):
    """Test Linear print output with simple terms."""
    linear = Linear(terms={1: 2.0, 2: 3.0}, constant=1.5)
    assert str(linear) == snapshot


def test_linear_print_empty(snapshot):
    """Test Linear print output with no terms."""
    linear = Linear(terms={}, constant=0.0)
    assert str(linear) == snapshot


def test_linear_print_constant_only(snapshot):
    """Test Linear print output with constant only."""
    linear = Linear(terms={}, constant=5.0)
    assert str(linear) == snapshot


def test_linear_print_negative_coefficients(snapshot):
    """Test Linear print output with negative coefficients."""
    linear = Linear(terms={1: -2.5, 3: 4.0, 5: -1.0}, constant=-3.0)
    assert str(linear) == snapshot


def test_function_print_linear(snapshot):
    """Test Function print output with linear function."""
    linear = Linear(terms={1: 2.0, 2: 3.0}, constant=1.0)
    function = Function(linear)
    assert str(function) == snapshot


def test_function_print_quadratic(snapshot):
    """Test Function print output with quadratic function."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    quadratic = x1 * x2 + 2 * x1 + 3 * x2 + 4
    function = Function(quadratic)
    assert str(function) == snapshot


def test_function_print_polynomial(snapshot):
    """Test Function print output with polynomial function."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)
    # Create a cubic polynomial: x1*x2*x3 + x1*x2 + x1
    polynomial = x1 * x2 * x3 + x1 * x2 + x1
    function = Function(polynomial)
    assert str(function) == snapshot


def test_constraint_print_equality(snapshot):
    """Test Constraint print output with equality constraint."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    constraint = (x1 + 2 * x2 == 5).set_id(1).add_name("equality_constraint")
    assert str(constraint) == snapshot


def test_constraint_print_less_equal(snapshot):
    """Test Constraint print output with less-than-or-equal constraint."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    constraint = (x1 + 2 * x2 <= 10).set_id(2).add_name("leq_constraint")
    assert str(constraint) == snapshot


def test_constraint_print_greater_equal(snapshot):
    """Test Constraint print output with greater-than-or-equal constraint."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)
    constraint = (x1 + x2 + x3 >= 1).set_id(3).add_name("geq_constraint")
    assert str(constraint) == snapshot


def test_instance_print_simple(snapshot):
    """Test Instance print output with simple problem."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    assert str(instance) == snapshot


def test_instance_print_with_constraints(snapshot):
    """Test Instance print output with constraints."""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + 2 * x[1] + 3 * x[2],
        constraints=[
            (x[0] + x[1] <= 1).set_id(0).add_name("max_two"),
            (x[1] + x[2] >= 1).set_id(1).add_name("min_one"),
            (x[0] + x[2] == 1).set_id(2).add_name("exactly_one"),
        ],
        sense=Instance.MINIMIZE,
    )
    assert str(instance) == snapshot


def test_instance_print_continuous_variables(snapshot):
    """Test Instance print output with continuous decision variables."""
    x = [DecisionVariable.continuous(i, lower=0.0, upper=10.0) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[(x[0] + 2 * x[1] <= 5).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    assert str(instance) == snapshot


def test_instance_print_integer_variables(snapshot):
    """Test Instance print output with integer decision variables."""
    x = [DecisionVariable.integer(i, lower=0, upper=100) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=3 * x[0] + 2 * x[1],
        constraints=[
            (2 * x[0] + x[1] <= 10).set_id(0).add_name("resource"),
        ],
        sense=Instance.MAXIMIZE,
    )
    assert str(instance) == snapshot


def test_decision_variable_analysis_print(snapshot):
    """Test DecisionVariableAnalysis print output."""
    x = [DecisionVariable.binary(i, name="x") for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[(x[1] + x[2] == 1).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    analysis = instance.decision_variable_analysis()
    assert str(analysis) == snapshot


def test_bound_print(snapshot):
    """Test Bound print output."""
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    analysis = instance.decision_variable_analysis()
    binary_vars = analysis.used_binary()
    bound = binary_vars[0]
    assert str(bound) == snapshot


def test_instance_stats_print(snapshot):
    """Test instance.stats() output."""
    x = [
        DecisionVariable.binary(0, name="x", subscripts=[0]),
        DecisionVariable.binary(1, name="x", subscripts=[1]),
        DecisionVariable.integer(2, lower=0, upper=10, name="y"),
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[
            (x[0] + x[1] <= 1).set_id(0),
            (x[1] + x[2] >= 1).set_id(1),
        ],
        sense=Instance.MINIMIZE,
    )
    stats = instance.stats()
    assert str(stats) == snapshot


def test_decision_variable_analysis_to_dict(snapshot):
    """Test DecisionVariableAnalysis.to_dict() output."""
    x = [DecisionVariable.binary(i, name="x") for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints=[(x[1] + x[2] == 1).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    analysis = instance.decision_variable_analysis()
    assert str(analysis.to_dict()) == snapshot
