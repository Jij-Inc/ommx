"""Collection-independent snapshots of Instance-owned state."""

from ommx import (
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
)


def test_empty_instance_read_apis():
    instance = Instance.minimize()
    before = instance.to_v2_bytes()
    assert instance.fixed_decision_variables() == {}
    assert instance.regular_constraint_ids() == set()
    assert instance.to_v2_bytes() == before


def test_fixed_decision_variables_is_an_independent_snapshot():
    x = DecisionVariable.continuous(2)
    y = DecisionVariable.continuous(19)
    bounded = DecisionVariable.continuous(50, lower=4, upper=4)
    free = DecisionVariable.continuous(91)
    original = Instance.from_components(
        decision_variables=[y, bounded, free, x],
        objective=x + y + bounded + free,
        constraints={},
        sense=Sense.Minimize,
    )
    assert original.fixed_decision_variables() == {}
    instance = original.partial_evaluate({19: -2.5, 2: 0})
    before = instance.to_v2_bytes()
    fixed = instance.fixed_decision_variables()
    assert isinstance(fixed, dict)
    assert fixed == {2: 0.0, 19: -2.5}
    assert all(
        type(key) is int and type(value) is float for key, value in fixed.items()
    )
    fixed[2] = 99.0
    del fixed[19]
    fixed[91] = 1.0
    assert instance.fixed_decision_variables() == {2: 0.0, 19: -2.5}
    assert instance.to_v2_bytes() == before
    assert original.fixed_decision_variables() == {}

    # Fixed values belong to the host, not to detached variable snapshots.
    attached = instance.attached_decision_variable(19)
    assert attached.substituted_value == -2.5
    detached = attached.detach()
    assert isinstance(detached, DecisionVariable)
    assert not hasattr(detached, "substituted_value")
    assert not hasattr(instance.get_decision_variable_by_id(19), "substituted_value")
    detached_instance = Instance.from_components(
        decision_variables=[detached],
        objective=detached,
        constraints={},
        sense=Sense.Minimize,
    )
    assert detached_instance.fixed_decision_variables() == {}


def test_regular_constraint_ids_tracks_active_regular_constraints():
    x = DecisionVariable.binary(2)
    y = DecisionVariable.binary(19)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints={7: x + y <= 1, 42: x - y == 0},
        one_hot_constraints={101: OneHotConstraint(variables=[x, y])},
        sos1_constraints={102: Sos1Constraint(variables=[x, y])},
        indicator_constraints={
            103: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    ids = instance.regular_constraint_ids()
    assert isinstance(ids, set)
    assert ids == {7, 42}
    assert all(type(key) is int for key in ids)
    assert instance.get_constraint_by_id(7).function.almost_equal(Function(x + y - 1))
    assert instance.get_constraint_by_id(42).function.almost_equal(Function(x - y))
    ids.remove(7)
    ids.add(999)
    assert instance.regular_constraint_ids() == {7, 42}
    assert instance.to_v2_bytes() == before

    added = instance.add_constraint(x <= 1)
    assert instance.regular_constraint_ids() == {7, 42, added.constraint_id}
    instance.relax_constraint(7, "test")
    assert instance.regular_constraint_ids() == {42, added.constraint_id}
    assert 7 in instance.removed_constraints
    instance.restore_constraint(7)
    assert instance.regular_constraint_ids() == {7, 42, added.constraint_id}
