"""Test decision-variable role and used-variable APIs."""

from ommx.v1 import DecisionVariable, DecisionVariableRole, Instance


def test_decision_variable_roles_basic():
    """Test basic decision-variable role query functionality."""
    x = [DecisionVariable.binary(i, name="x") for i in range(3)]

    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1],
        constraints={0: x[1] + x[2] == 1},
        sense=Instance.MAXIMIZE,
    )

    assert instance.used_decision_variables == x
    assert instance.decision_variable_roles() == {
        0: DecisionVariableRole.Used,
        1: DecisionVariableRole.Used,
        2: DecisionVariableRole.Used,
    }
    assert instance.decision_variable_role(0) == DecisionVariableRole.Used
    assert instance.decision_variable_role(999) is None


def test_decision_variable_role_partitions():
    """Test used/fixed/dependent/irrelevant role partitions on Instance."""
    x = {i: DecisionVariable.continuous(i) for i in range(4)}
    instance = Instance.from_components(
        decision_variables=list(x.values()),
        objective=x[0],
        constraints={},
        sense=Instance.MINIMIZE,
    )
    instance.substitute({2: x[0] + 1})
    instance = instance.partial_evaluate({1: 2.0})

    assert instance.decision_variable_roles() == {
        0: DecisionVariableRole.Used,
        1: DecisionVariableRole.Fixed,
        2: DecisionVariableRole.Dependent,
        3: DecisionVariableRole.Irrelevant,
    }
    assert instance.fixed_decision_variables() == {1: 2.0}
    assert instance.dependent_decision_variable_ids() == {2}
    assert instance.irrelevant_decision_variable_ids() == {3}


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
