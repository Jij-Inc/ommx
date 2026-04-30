"""Tests for AttachedOneHotConstraint and AttachedSos1Constraint write-through wrappers.

These two share the same shape (structural constraints over a `variables` set
plus metadata), so they are tested together. The bulk of the coverage focuses
on metadata write-through; kind-specific behavior is just `variables`.
"""

import copy

import pytest

from ommx.v1 import (
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    DecisionVariable,
    Function,
    Instance,
    Linear,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
    Sense,
    Sos1Constraint,
)


def _empty_instance() -> Instance:
    return Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[
            DecisionVariable.binary(0),
            DecisionVariable.binary(1),
            DecisionVariable.binary(2),
        ],
        constraints={},
    )


def _empty_parametric_instance() -> ParametricInstance:
    return ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[
            DecisionVariable.binary(0),
            DecisionVariable.binary(1),
            DecisionVariable.binary(2),
        ],
        constraints={},
        parameters=[Parameter(id=100, name="alpha")],
    )


# --------------------------------------------------------------------------- #
# OneHotConstraint                                                            #
# --------------------------------------------------------------------------- #


def test_one_hot_add_returns_attached_with_drained_metadata():
    instance = _empty_instance()
    snapshot = OneHotConstraint(
        variables=[0, 1, 2],
        name="pick_one",
        subscripts=[3],
        description="initial",
        parameters={"k": "v"},
    )

    attached = instance.add_one_hot_constraint(snapshot)

    assert isinstance(attached, AttachedOneHotConstraint)
    assert attached.name == "pick_one"
    assert attached.subscripts == [3]
    assert attached.description == "initial"
    assert attached.parameters == {"k": "v"}
    assert attached.variables == [0, 1, 2]


def test_one_hot_setter_writes_through_to_instance():
    instance = _empty_instance()
    attached = instance.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1, 2], name="pick_one")
    )
    cid = attached.constraint_id

    attached.set_name("renamed")
    attached.set_subscripts([7, 8])
    attached.set_description("updated")
    attached.set_parameters({"a": "1"})

    fresh = instance.one_hot_constraints[cid]
    assert fresh.name == "renamed"
    assert fresh.subscripts == [7, 8]
    assert fresh.description == "updated"
    assert fresh.parameters == {"a": "1"}


def test_one_hot_two_handles_share_state():
    instance = _empty_instance()
    a = instance.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1], name="pick_one")
    )
    b = instance.one_hot_constraints[a.constraint_id]

    a.set_name("renamed")
    assert b.name == "renamed"


def test_one_hot_constraints_getter_returns_attached():
    instance = _empty_instance()
    a = instance.add_one_hot_constraint(OneHotConstraint(variables=[0, 1], name="c1"))
    b = instance.add_one_hot_constraint(OneHotConstraint(variables=[1, 2], name="c2"))

    constraints = instance.one_hot_constraints
    assert set(constraints.keys()) == {a.constraint_id, b.constraint_id}
    assert all(isinstance(c, AttachedOneHotConstraint) for c in constraints.values())


def test_one_hot_detach_returns_independent_snapshot():
    instance = _empty_instance()
    attached = instance.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1], name="pick_one")
    )

    snapshot = attached.detach()
    assert isinstance(snapshot, OneHotConstraint)
    assert snapshot.name == "pick_one"


def test_one_hot_keeps_instance_alive_after_del():
    instance = _empty_instance()
    attached = instance.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1], name="pick_one")
    )

    del instance

    assert attached.name == "pick_one"
    attached.set_name("renamed")
    assert attached.name == "renamed"


def test_one_hot_add_rejects_undefined_variable():
    instance = _empty_instance()  # variables = [0, 1, 2]
    bad = OneHotConstraint(variables=[0, 99])

    with pytest.raises(Exception, match="99"):
        instance.add_one_hot_constraint(bad)

    assert instance.one_hot_constraints == {}


def test_one_hot_copy_and_deepcopy_share_parent():
    instance = _empty_instance()
    attached = instance.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1], name="pick_one")
    )

    shallow = copy.copy(attached)
    deep = copy.deepcopy(attached)

    assert shallow.constraint_id == attached.constraint_id
    assert deep.constraint_id == attached.constraint_id
    assert shallow.instance is instance
    assert deep.instance is instance


def test_one_hot_on_parametric_host():
    parametric = _empty_parametric_instance()
    attached = parametric.add_one_hot_constraint(
        OneHotConstraint(variables=[0, 1, 2], name="pick_one")
    )

    assert isinstance(attached, AttachedOneHotConstraint)
    assert attached.instance is parametric

    attached.set_name("renamed")
    fresh = parametric.one_hot_constraints[attached.constraint_id]
    assert fresh.name == "renamed"


# --------------------------------------------------------------------------- #
# Sos1Constraint                                                              #
# --------------------------------------------------------------------------- #


def test_sos1_add_returns_attached_with_drained_metadata():
    instance = _empty_instance()
    snapshot = Sos1Constraint(
        variables=[0, 1, 2],
        name="exclusive",
        subscripts=[5],
    )

    attached = instance.add_sos1_constraint(snapshot)

    assert isinstance(attached, AttachedSos1Constraint)
    assert attached.name == "exclusive"
    assert attached.subscripts == [5]
    assert attached.variables == [0, 1, 2]


def test_sos1_setter_writes_through_to_instance():
    instance = _empty_instance()
    attached = instance.add_sos1_constraint(
        Sos1Constraint(variables=[0, 1, 2], name="exclusive")
    )
    cid = attached.constraint_id

    attached.set_name("renamed")
    attached.set_subscripts([10])

    fresh = instance.sos1_constraints[cid]
    assert fresh.name == "renamed"
    assert fresh.subscripts == [10]


def test_sos1_constraints_getter_returns_attached():
    instance = _empty_instance()
    instance.add_sos1_constraint(Sos1Constraint(variables=[0, 1], name="c1"))
    instance.add_sos1_constraint(Sos1Constraint(variables=[1, 2], name="c2"))

    constraints = instance.sos1_constraints
    assert all(isinstance(c, AttachedSos1Constraint) for c in constraints.values())


def test_sos1_detach_returns_independent_snapshot():
    instance = _empty_instance()
    attached = instance.add_sos1_constraint(
        Sos1Constraint(variables=[0, 1], name="exclusive")
    )

    snapshot = attached.detach()
    assert isinstance(snapshot, Sos1Constraint)
    assert snapshot.name == "exclusive"


def test_sos1_add_rejects_undefined_variable():
    instance = _empty_instance()
    bad = Sos1Constraint(variables=[0, 99])

    with pytest.raises(Exception, match="99"):
        instance.add_sos1_constraint(bad)


def test_sos1_on_parametric_host():
    parametric = _empty_parametric_instance()
    attached = parametric.add_sos1_constraint(
        Sos1Constraint(variables=[0, 1, 2], name="exclusive")
    )

    assert isinstance(attached, AttachedSos1Constraint)
    assert attached.instance is parametric

    attached.set_subscripts([3])
    fresh = parametric.sos1_constraints[attached.constraint_id]
    assert fresh.subscripts == [3]


# --------------------------------------------------------------------------- #
# Structural-position validation on ParametricInstance                        #
# --------------------------------------------------------------------------- #


def test_one_hot_on_parametric_rejects_parameter_id_in_variables():
    """OneHot's `variables` set is a structural position — substitution can't
    fill it later. A parameter id there must be rejected."""
    parametric = _empty_parametric_instance()  # parameters = [100]
    bad = OneHotConstraint(variables=[0, 100])

    with pytest.raises(Exception, match="(structural|parameter)"):
        parametric.add_one_hot_constraint(bad)

    assert parametric.one_hot_constraints == {}


def test_sos1_on_parametric_rejects_parameter_id_in_variables():
    """Same structural-position rule for SOS1."""
    parametric = _empty_parametric_instance()  # parameters = [100]
    bad = Sos1Constraint(variables=[0, 100])

    with pytest.raises(Exception, match="(structural|parameter)"):
        parametric.add_sos1_constraint(bad)

    assert parametric.sos1_constraints == {}
