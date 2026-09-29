"""Collection-independent snapshots shared with the Python SDK 3 API."""

from ommx import DecisionVariable, Function, Instance
from ommx.v1 import ConstraintHints, OneHot, Sos1


def test_empty_instance_read_apis():
    instance = Instance.from_components(
        decision_variables=[], objective=0, constraints=[], sense=Instance.MINIMIZE
    )
    before = instance.to_bytes()
    assert instance.fixed_decision_variables() == {}
    assert instance.regular_constraint_ids() == set()
    assert instance.to_bytes() == before


def test_fixed_decision_variables_is_an_independent_snapshot():
    x = DecisionVariable.continuous(2)
    y = DecisionVariable.continuous(19)
    bounded = DecisionVariable.continuous(50, lower=4, upper=4)
    free = DecisionVariable.continuous(91)
    original = Instance.from_components(
        decision_variables=[y, bounded, free, x],
        objective=x + y + bounded + free,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    assert original.fixed_decision_variables() == {}
    instance = original.partial_evaluate({19: -2.5, 2: 0})
    before = instance.to_bytes()
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
    assert instance.to_bytes() == before
    assert original.fixed_decision_variables() == {}
    assert Instance.from_bytes(before).fixed_decision_variables() == {2: 0.0, 19: -2.5}


def test_regular_constraint_ids_tracks_relaxation_and_restoration():
    x = DecisionVariable.binary(2)
    y = DecisionVariable.binary(19)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints=[(x + y <= 1).set_id(7), (x - y == 0).set_id(42)],
        sense=Instance.MINIMIZE,
    )
    before = instance.to_bytes()
    ids = instance.regular_constraint_ids()
    assert isinstance(ids, set)
    assert ids == {7, 42}
    assert all(type(key) is int for key in ids)
    assert instance.get_constraint_by_id(7).function.almost_equal(Function(x + y - 1))
    assert instance.get_constraint_by_id(42).function.almost_equal(Function(x - y))
    ids.remove(7)
    ids.add(999)
    assert instance.regular_constraint_ids() == {7, 42}
    assert instance.to_bytes() == before

    instance.relax_constraint(7, "test")
    assert instance.regular_constraint_ids() == {42}
    assert instance.get_removed_constraint_by_id(7).id == 7
    instance.restore_constraint(7)
    assert instance.regular_constraint_ids() == {7, 42}


def test_regular_constraint_ids_includes_sources_of_hints():
    x = DecisionVariable.binary(2)
    y = DecisionVariable.binary(19)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints=[(x + y == 1).set_id(7), (x + y <= 1).set_id(42)],
        sense=Instance.MINIMIZE,
        constraint_hints=ConstraintHints(
            one_hot_constraints=[OneHot(id=7, variables=[2, 19])],
            sos1_constraints=[
                Sos1(
                    binary_constraint_id=42, variables=[2, 19], big_m_constraint_ids=[]
                )
            ],
        ),
    )
    before = instance.to_bytes()
    assert instance.regular_constraint_ids() == {7, 42}
    assert instance.get_constraint_by_id(7).function.almost_equal(Function(x + y - 1))
    assert instance.get_constraint_by_id(42).function.almost_equal(Function(x + y - 1))
    assert instance.to_bytes() == before
