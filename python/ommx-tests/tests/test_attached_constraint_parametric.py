"""Tests for AttachedConstraint write-through against a ParametricInstance host."""

import copy

import pytest

from ommx.v1 import (
    AttachedConstraint,
    Constraint,
    DecisionVariable,
    Equality,
    Function,
    Linear,
    Parameter,
    ParametricInstance,
    Sense,
)


def _empty_parametric_instance() -> ParametricInstance:
    return ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[
            DecisionVariable.binary(0),
            DecisionVariable.binary(1),
        ],
        constraints={},
        parameters=[Parameter(id=100, name="alpha")],
    )


def _make_constraint(name: str | None = "balance") -> Constraint:
    linear = Linear({0: 1.0, 1: 1.0}, -1.0)
    return Constraint(
        function=Function.from_linear(linear),
        equality=Equality.EqualToZero,
        name=name,
        subscripts=[7],
        description="initial",
        parameters={"k": "v"},
    )


def test_add_constraint_returns_attached_with_drained_metadata():
    """ParametricInstance.add_constraint mirrors Instance: returns an
    AttachedConstraint reading the staged metadata."""
    parametric = _empty_parametric_instance()
    snapshot = _make_constraint(name="balance")

    attached = parametric.add_constraint(snapshot)

    assert isinstance(attached, AttachedConstraint)
    assert attached.name == "balance"
    assert attached.subscripts == [7]
    assert attached.description == "initial"
    assert attached.parameters == {"k": "v"}


def test_attached_setter_writes_through_to_parametric_instance():
    """Mutations on the AttachedConstraint land in the parent parametric
    instance and surface via parametric.constraints[id]."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))
    cid = attached.constraint_id

    attached.set_name("demand")
    attached.set_subscripts([1, 2, 3])
    attached.set_description("updated")
    attached.set_parameters({"a": "1", "b": "2"})

    fresh = parametric.constraints[cid]
    assert fresh.name == "demand"
    assert fresh.subscripts == [1, 2, 3]
    assert fresh.description == "updated"
    assert fresh.parameters == {"a": "1", "b": "2"}


def test_two_attached_handles_share_state_on_parametric():
    """Two AttachedConstraint handles for the same id on a parametric host
    observe the same data."""
    parametric = _empty_parametric_instance()
    a = parametric.add_constraint(_make_constraint(name="balance"))
    b = parametric.constraints[a.constraint_id]

    a.set_name("demand")

    assert b.name == "demand"


def test_constraints_getter_returns_attached_constraints():
    """parametric.constraints values are AttachedConstraint, not Constraint."""
    parametric = _empty_parametric_instance()
    a = parametric.add_constraint(_make_constraint(name="c1"))
    b = parametric.add_constraint(_make_constraint(name="c2"))

    constraints = parametric.constraints
    assert set(constraints.keys()) == {a.constraint_id, b.constraint_id}
    assert all(isinstance(c, AttachedConstraint) for c in constraints.values())


def test_detach_returns_independent_constraint_snapshot():
    """detach() on a parametric-hosted handle returns an independent
    Constraint snapshot."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))

    snapshot = attached.detach()
    assert isinstance(snapshot, Constraint)
    assert snapshot.name == "balance"

    snapshot.set_name("ignored")
    assert attached.name == "balance"


def test_attached_constraint_id_and_parametric_instance_handle():
    """constraint_id surfaces the assigned id and instance returns the parent
    ParametricInstance."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))

    assert isinstance(attached.constraint_id, int)
    assert attached.instance is parametric


def test_attached_keeps_parametric_instance_alive_after_del():
    """`del parametric` only drops one Python binding; the AttachedConstraint
    still holds a Py<ParametricInstance> refcount, so the underlying parametric
    instance — and the SoA store backing read/write-through — remain valid."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))

    del parametric

    assert attached.name == "balance"
    attached.set_name("demand")
    assert attached.name == "demand"
    snapshot = attached.detach()
    assert snapshot.name == "demand"


def test_add_constraint_accepts_parameter_reference():
    """A constraint that references a parameter id (not a decision variable)
    is accepted on a ParametricInstance host — this is the key validation
    difference vs. Instance.add_constraint."""
    parametric = _empty_parametric_instance()
    # parameter id = 100 is registered; this would fail on a regular Instance.
    parametric_constraint = Constraint(
        function=Function.from_linear(Linear({0: 1.0, 100: -1.0}, 0.0)),
        equality=Equality.EqualToZero,
        name="param_link",
    )

    attached = parametric.add_constraint(parametric_constraint)
    assert attached.name == "param_link"


def test_add_constraint_rejects_undefined_id():
    """ParametricInstance.add_constraint validates that all referenced ids
    are defined as either a decision variable or a parameter."""
    parametric = _empty_parametric_instance()  # variables [0, 1], parameters [100]
    rogue = Linear({999: 1.0}, 0.0)  # neither variable nor parameter
    bad = Constraint(
        function=Function.from_linear(rogue),
        equality=Equality.EqualToZero,
    )

    with pytest.raises(Exception, match="999"):
        parametric.add_constraint(bad)

    assert parametric.constraints == {}


def test_evaluate_rejected_on_parametric_host():
    """AttachedConstraint.evaluate is meaningful only on Instance hosts; a
    parametric-hosted constraint may still reference unsubstituted parameters,
    so evaluate() raises rather than silently producing a misleading value."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))

    with pytest.raises(Exception, match="ParametricInstance"):
        attached.evaluate({0: 1.0, 1: 0.0})


def test_copy_and_deepcopy_share_parametric_parent():
    """copy / deepcopy of an AttachedConstraint on a parametric host produce
    another handle pointing at the same id on the same ParametricInstance."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_constraint(_make_constraint(name="balance"))

    shallow = copy.copy(attached)
    deep = copy.deepcopy(attached)

    assert shallow.constraint_id == attached.constraint_id
    assert deep.constraint_id == attached.constraint_id
    assert shallow.instance is parametric
    assert deep.instance is parametric

    attached.set_name("demand")
    assert shallow.name == "demand"
    assert deep.name == "demand"
