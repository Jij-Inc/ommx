"""Snapshot tests for print output of OMMX v1 classes."""

from ommx import (
    Constraint,
    DecisionVariable,
    IndicatorConstraint,
    Linear,
    Function,
    Instance,
    NamedFunction,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
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


def test_function_print_tiny_nonzero_coefficient(snapshot):
    """Test Function print output preserves representable nonzero coefficients."""
    x1 = DecisionVariable.binary(1)
    function = Function(1e-20 * x1)
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
    constraint = (x1 + 2 * x2 == 5).set_name("equality_constraint")
    assert str(constraint) == snapshot


def test_constraint_print_less_equal(snapshot):
    """Test Constraint print output with less-than-or-equal constraint."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    constraint = (x1 + 2 * x2 <= 10).set_name("leq_constraint")
    assert str(constraint) == snapshot


def test_constraint_print_greater_equal(snapshot):
    """Test Constraint print output with greater-than-or-equal constraint."""
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)
    constraint = (x1 + x2 + x3 >= 1).set_name("geq_constraint")
    assert str(constraint) == snapshot


def test_bound_print(snapshot):
    """Test Bound print output."""
    x = [DecisionVariable.binary(i) for i in range(2)]
    bound = x[0].bound
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
        constraints={0: x[0] + x[1] <= 1, 1: x[1] + x[2] >= 1},
        sense=Instance.MINIMIZE,
    )
    stats = instance.stats()
    assert str(stats) == snapshot


def test_instance_print_uses_modeling_labels(snapshot):
    """Test Instance print output with context-aware function formatting."""
    x = [
        DecisionVariable.binary(0, name="x", subscripts=[0]),
        DecisionVariable.binary(1, name="x", subscripts=[1]),
        DecisionVariable.integer(2, lower=0, upper=10, name="y"),
    ]
    capacity = Constraint(
        function=x[0] + x[1] - 1,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
        name="capacity",
        subscripts=[0],
    )
    indicator = IndicatorConstraint(
        indicator_variable=x[0],
        function=x[1] - 1,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
        name="active",
    )
    one_hot = OneHotConstraint(variables=x[:2], name="choose")
    score = NamedFunction(id=5, function=2 * x[2] + 3, name="score")
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints={10: capacity},
        indicator_constraints={20: indicator},
        one_hot_constraints={30: one_hot},
        named_functions=[score],
        sense=Instance.MAXIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)


def test_parametric_instance_print_uses_parameter_labels(snapshot):
    """Test ParametricInstance print output with parameter labels."""
    x = DecisionVariable.binary(0, name="x")
    p = Parameter(100, name="p", parameters={"scenario": "base"})
    instance = ParametricInstance.from_components(
        decision_variables=[x],
        objective=x + p,
        constraints={},
        parameters=[p],
        sense=Instance.MINIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)


def test_instance_print_disambiguates_duplicate_labels_across_sections(snapshot):
    """Test duplicate labels stay unambiguous across summary sections."""
    x0 = DecisionVariable.binary(0, name="x")
    x1 = DecisionVariable.binary(1, name="x")
    constraint = Constraint(
        function=x1 - 1,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
        name="limit",
    )
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0,
        constraints={1: constraint},
        sense=Instance.MINIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)


def test_parametric_instance_print_disambiguates_parameter_and_structural_variable(
    snapshot,
):
    """Test parameter labels share the summary namespace with structural variables."""
    x = DecisionVariable.binary(0, name="shared")
    p = Parameter(100, name="shared")
    one_hot = OneHotConstraint(variables=[x], name="choose")
    instance = ParametricInstance.from_components(
        decision_variables=[x],
        objective=p,
        constraints={},
        parameters=[p],
        one_hot_constraints={7: one_hot},
        sense=Instance.MINIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)


def test_instance_print_limits_section_rows(snapshot):
    """Test large sections stay bounded for repr/notebook display."""
    variables = [
        DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(25)
    ]
    constraints = {
        i: Constraint(
            function=variables[i] - 1,
            equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            name="row",
            subscripts=[i],
        )
        for i in range(22)
    }
    instance = Instance.from_components(
        decision_variables=variables,
        objective=variables[0],
        constraints=constraints,
        sense=Instance.MINIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)


def test_instance_print_limits_structural_variable_sets(snapshot):
    """Test large one-hot/SOS1-style variable sets stay bounded."""
    variables = [
        DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(25)
    ]
    one_hot = OneHotConstraint(variables=variables, name="choose")
    instance = Instance.from_components(
        decision_variables=variables,
        objective=variables[0],
        constraints={},
        one_hot_constraints={3: one_hot},
        sense=Instance.MINIMIZE,
    )

    assert str(instance) == snapshot
    assert repr(instance) == str(instance)
