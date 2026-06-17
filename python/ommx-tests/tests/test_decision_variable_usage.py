"""Test decision_variable_usage API implementation."""

from ommx.v1 import DecisionVariable, DecisionVariableRole, Instance


def test_decision_variable_usage_basic():
    """Test basic decision_variable_usage functionality."""
    # Create binary variables
    x = [DecisionVariable.binary(i, name="x") for i in range(3)]

    # Create instance with objective and constraints
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints={0: x[1] + x[2] == 1},
        sense=Instance.MAXIMIZE,
    )

    usage = instance.decision_variable_usage()

    # Test basic functionality
    used_ids = usage.used_decision_variable_ids()
    assert used_ids == {0, 1, 2}

    # Test used_in_objective
    objective_vars = usage.used_in_objective()
    assert objective_vars == {0, 1}

    # Test used_in_constraints
    constraint_vars = usage.used_in_constraints()
    assert 0 in constraint_vars
    assert constraint_vars[0] == {1, 2}

    assert usage.roles() == {
        0: DecisionVariableRole.Used,
        1: DecisionVariableRole.Used,
        2: DecisionVariableRole.Used,
    }

    by_variable = usage.by_variable()
    assert by_variable[0].role == DecisionVariableRole.Used
    assert by_variable[0].used_in_objective
    assert by_variable[2].used_in_regular_constraints() == {0}


def test_bound_wrapper_functionality():
    """Test that Bound wrapper works correctly."""
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    bound = x[0].bound

    # Test bound methods
    assert bound.lower == 0.0
    assert bound.upper == 1.0
    assert bound.width() == 1.0
    assert bound.is_finite()
    assert bound.contains(0.5, 0.001)
    assert not bound.contains(-0.1, 0.001)
    assert bound.nearest_to_zero() == 0.0

    # Test string representations
    assert "0" in str(bound)
    assert "1" in str(bound)


def test_instance_populate_state():
    """Test that Instance owns state population for fixed/dependent variables."""
    x = {i: DecisionVariable.continuous(i) for i in [1, 2, 5, 10, 99]}
    instance = Instance.from_components(
        decision_variables=list(x.values()),
        objective=x[1] + x[2],
        constraints={},
        sense=Instance.MINIMIZE,
    )
    instance.substitute(
        {
            10: x[1] + x[2],
            5: x[10] + 1,
        }
    )
    instance = instance.partial_evaluate({99: 4.0})

    populated = instance.populate_state({1: 2.0, 2: 3.0})

    assert populated.entries == {
        1: 2.0,
        2: 3.0,
        5: 6.0,
        10: 5.0,
        99: 4.0,
    }
