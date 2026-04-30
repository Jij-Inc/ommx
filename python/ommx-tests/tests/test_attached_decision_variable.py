"""Tests for AttachedDecisionVariable write-through wrapper."""

import copy

import pytest

from ommx.v1 import (
    AttachedDecisionVariable,
    DecisionVariable,
    Function,
    Instance,
    Linear,
    Parameter,
    ParametricInstance,
    Sense,
)


def _empty_instance() -> Instance:
    return Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[DecisionVariable.binary(0)],
        constraints={},
    )


def _empty_parametric_instance() -> ParametricInstance:
    return ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[DecisionVariable.binary(0)],
        constraints={},
        parameters=[Parameter(id=100, name="alpha")],
    )


def _make_variable() -> DecisionVariable:
    return DecisionVariable.integer(
        7,
        lower=0,
        upper=10,
        name="x_demand",
        subscripts=[3],
        description="initial",
        parameters={"k": "v"},
    )


# --- Instance host ---


def test_add_returns_attached_with_drained_metadata():
    instance = _empty_instance()
    variable = _make_variable()

    attached = instance.add_decision_variable(variable)

    assert isinstance(attached, AttachedDecisionVariable)
    assert attached.id == 7
    assert attached.name == "x_demand"
    assert attached.subscripts == [3]
    assert attached.description == "initial"
    assert attached.parameters == {"k": "v"}


def test_setter_writes_through_to_instance_soa_store():
    instance = _empty_instance()
    attached = instance.add_decision_variable(_make_variable())

    attached.set_name("renamed")
    attached.set_subscripts([1, 2])
    attached.set_description("updated")
    attached.set_parameters({"a": "1"})

    # Read-back through a fresh handle resolved by id.
    fresh = instance.attached_decision_variable(attached.id)
    assert fresh.name == "renamed"
    assert fresh.subscripts == [1, 2]
    assert fresh.description == "updated"
    assert fresh.parameters == {"a": "1"}


def test_attached_decision_variable_lookup_returns_handle():
    """attached_decision_variable(id) returns a write-through handle for an
    existing variable; missing id raises KeyError."""
    instance = _empty_instance()
    instance.add_decision_variable(_make_variable())

    handle = instance.attached_decision_variable(7)
    assert isinstance(handle, AttachedDecisionVariable)
    assert handle.id == 7
    assert handle.name == "x_demand"

    with pytest.raises(KeyError):
        instance.attached_decision_variable(999)


def test_decision_variables_getter_still_returns_snapshots():
    """The plain decision_variables getter preserves snapshot semantics so
    arithmetic (`x + y` style expression building) keeps working."""
    instance = _empty_instance()
    instance.add_decision_variable(_make_variable())

    variables = instance.decision_variables
    assert all(isinstance(v, DecisionVariable) for v in variables)
    # Sanity: arithmetic compiles and produces a Linear.
    x, y = variables[0], variables[1]
    expr = x + y
    assert hasattr(expr, "almost_equal")


def test_two_handles_share_state():
    instance = _empty_instance()
    a = instance.add_decision_variable(_make_variable())
    b = instance.attached_decision_variable(a.id)

    a.set_name("renamed")
    assert b.name == "renamed"


def test_detach_returns_independent_snapshot():
    instance = _empty_instance()
    attached = instance.add_decision_variable(_make_variable())

    snapshot = attached.detach()
    assert isinstance(snapshot, DecisionVariable)
    assert snapshot.name == "x_demand"
    assert snapshot.subscripts == [3]


def test_attached_keeps_instance_alive_after_del():
    instance = _empty_instance()
    attached = instance.add_decision_variable(_make_variable())

    del instance

    assert attached.name == "x_demand"
    attached.set_name("renamed")
    assert attached.name == "renamed"


def test_add_rejects_duplicate_id():
    instance = _empty_instance()  # variable id 0 already there
    duplicate = DecisionVariable.binary(0)

    with pytest.raises(Exception, match="Duplicate"):
        instance.add_decision_variable(duplicate)


def test_kind_and_bound_getters_read_through():
    instance = _empty_instance()
    attached = instance.add_decision_variable(_make_variable())

    assert attached.kind == DecisionVariable.INTEGER
    bound = attached.bound
    assert bound.lower == 0
    assert bound.upper == 10


def test_copy_and_deepcopy_share_parent_instance():
    instance = _empty_instance()
    attached = instance.add_decision_variable(_make_variable())

    shallow = copy.copy(attached)
    deep = copy.deepcopy(attached)

    assert shallow.id == attached.id
    assert deep.id == attached.id
    assert shallow.instance is instance
    assert deep.instance is instance

    attached.set_name("renamed")
    assert shallow.name == "renamed"
    assert deep.name == "renamed"


# --- ParametricInstance host ---


def test_add_returns_attached_on_parametric_host():
    parametric = _empty_parametric_instance()
    attached = parametric.add_decision_variable(_make_variable())

    assert isinstance(attached, AttachedDecisionVariable)
    assert attached.instance is parametric
    assert attached.name == "x_demand"


def test_setter_writes_through_on_parametric_host():
    parametric = _empty_parametric_instance()
    attached = parametric.add_decision_variable(_make_variable())

    attached.set_name("renamed")
    attached.set_subscripts([1, 2, 3])

    fresh = parametric.attached_decision_variable(attached.id)
    assert fresh.name == "renamed"
    assert fresh.subscripts == [1, 2, 3]


def test_add_rejects_id_collision_with_parameter_on_parametric():
    """A new variable cannot collide with an existing parameter id."""
    parametric = _empty_parametric_instance()  # parameter id = 100
    bad = DecisionVariable.binary(100)

    with pytest.raises(Exception, match="parameter"):
        parametric.add_decision_variable(bad)
