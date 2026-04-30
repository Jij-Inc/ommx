"""Tests for AttachedConstraint write-through wrapper."""

import copy

import pytest

from ommx.v1 import (
    AttachedConstraint,
    Constraint,
    DecisionVariable,
    Equality,
    Function,
    Instance,
    Linear,
)


def _empty_instance() -> Instance:
    return Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[DecisionVariable.binary(0), DecisionVariable.binary(1)],
        constraints={},
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
    """add_constraint returns AttachedConstraint reading the staged metadata."""
    instance = _empty_instance()
    snapshot = _make_constraint(name="balance")

    attached = instance.add_constraint(snapshot)

    assert isinstance(attached, AttachedConstraint)
    assert attached.name == "balance"
    assert attached.subscripts == [7]
    assert attached.description == "initial"
    assert attached.parameters == {"k": "v"}


def test_add_constraint_does_not_mutate_input_snapshot():
    """The input Constraint stays a snapshot — write-through goes through the
    returned AttachedConstraint, not the original."""
    instance = _empty_instance()
    snapshot = _make_constraint(name="balance")

    attached = instance.add_constraint(snapshot)
    attached.set_name("demand")

    # Original snapshot is untouched.
    assert snapshot.name == "balance"
    # SoA store updated.
    assert attached.name == "demand"


def test_attached_setter_writes_through_to_instance():
    """Mutations on AttachedConstraint land in the parent instance and surface
    via instance.constraints[id]."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    cid = attached.constraint_id

    attached.set_name("demand")
    attached.set_subscripts([1, 2, 3])
    attached.set_description("updated")
    attached.set_parameters({"a": "1", "b": "2"})

    fresh = instance.constraints[cid]
    assert fresh.name == "demand"
    assert fresh.subscripts == [1, 2, 3]
    assert fresh.description == "updated"
    assert fresh.parameters == {"a": "1", "b": "2"}


def test_two_attached_handles_share_state():
    """Two AttachedConstraint handles for the same id observe the same data."""
    instance = _empty_instance()
    a = instance.add_constraint(_make_constraint(name="balance"))
    b = instance.constraints[a.constraint_id]

    a.set_name("demand")

    assert b.name == "demand"


def test_constraints_getter_returns_attached_constraints():
    """instance.constraints values are AttachedConstraint, not Constraint."""
    instance = _empty_instance()
    a = instance.add_constraint(_make_constraint(name="c1"))
    b = instance.add_constraint(_make_constraint(name="c2"))

    constraints = instance.constraints
    assert set(constraints.keys()) == {a.constraint_id, b.constraint_id}
    assert all(isinstance(c, AttachedConstraint) for c in constraints.values())


def test_detach_returns_independent_constraint_snapshot():
    """detach() returns a Constraint whose mutations do not propagate back."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))

    snapshot = attached.detach()
    assert isinstance(snapshot, Constraint)
    assert snapshot.name == "balance"

    snapshot.set_name("ignored")
    assert attached.name == "balance"


def test_attached_evaluate_uses_live_data():
    """evaluate() works on AttachedConstraint and yields the same value as on
    a detached snapshot."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    state = {0: 1.0, 1: 0.0}  # 1 + 0 - 1 = 0 → satisfies the equality

    evaluated = attached.evaluate(state)
    assert evaluated.evaluated_value == pytest.approx(0.0)
    assert evaluated.feasible is True


def test_attached_constraint_id_and_instance_handle():
    """constraint_id surfaces the assigned id and instance returns the parent."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))

    assert isinstance(attached.constraint_id, int)
    assert attached.instance is instance


def test_attached_keeps_instance_alive_after_del():
    """`del instance` only drops one Python binding; the AttachedConstraint
    still holds a Py<Instance> refcount, so the underlying instance — and the
    SoA store backing read/write-through — remain valid."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))

    del instance

    # Read-through still works.
    assert attached.name == "balance"
    # Write-through still works.
    attached.set_name("demand")
    assert attached.name == "demand"
    # detach() still finds the constraint and its metadata in the store.
    snapshot = attached.detach()
    assert snapshot.name == "demand"


def test_add_subscripts_extends_instead_of_replacing():
    """add_subscripts appends to the existing list (extend_subscripts on the
    SoA store), in contrast with set_subscripts which replaces."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    # _make_constraint stages [7] initially.
    assert attached.subscripts == [7]

    attached.add_subscripts([1, 2])

    assert attached.subscripts == [7, 1, 2]


def test_add_parameter_adds_single_key_without_clearing_others():
    """add_parameter writes a single (key, value) entry while leaving other
    parameters intact. add_parameters / set_parameters wholesale-replace."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    # _make_constraint stages {"k": "v"} initially.
    assert attached.parameters == {"k": "v"}

    attached.add_parameter("k2", "v2")

    assert attached.parameters == {"k": "v", "k2": "v2"}


def test_add_parameters_replaces_existing_parameter_dict():
    """add_parameters is an alias for set_parameters; it replaces the whole
    parameter map rather than merging entries."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    assert attached.parameters == {"k": "v"}

    attached.add_parameters({"a": "1", "b": "2"})

    assert attached.parameters == {"a": "1", "b": "2"}


def test_attached_after_relax_constraint_still_reads_through():
    """relax_constraint moves a constraint from active to removed; the
    AttachedConstraint handle stays valid because lookup_constraint checks
    both maps and the SoA metadata store is keyed by id regardless."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))
    cid = attached.constraint_id
    assert cid in instance.constraints

    instance.relax_constraint(cid, "manual")
    assert cid not in instance.constraints

    # The handle still resolves: getters fall through to the removed map for
    # core data and to the SoA store for metadata.
    assert attached.name == "balance"
    assert attached.subscripts == [7]
    snapshot = attached.detach()
    assert snapshot.name == "balance"


def test_add_constraint_rejects_undefined_variable():
    """add_constraint validates that all referenced variables already exist
    in the instance — same guarantee as insert_constraint. Referencing a
    variable that was never declared raises rather than leaving the
    instance in an invalid state."""
    instance = _empty_instance()  # decision_variables = [0, 1]
    rogue = Linear({99: 1.0}, 0.0)  # variable 99 is undefined
    bad_constraint = Constraint(
        function=Function.from_linear(rogue),
        equality=Equality.EqualToZero,
    )

    with pytest.raises(Exception, match="99"):
        instance.add_constraint(bad_constraint)

    # Instance is unchanged.
    assert instance.constraints == {}


def test_copy_and_deepcopy_share_the_same_parent_instance():
    """`copy.copy` / `copy.deepcopy` on AttachedConstraint produce another
    handle that points at the same id on the same parent Instance — the
    wrapper is a refcounted handle, not a value type. A write-through on
    one handle is observable on the copy."""
    instance = _empty_instance()
    attached = instance.add_constraint(_make_constraint(name="balance"))

    shallow = copy.copy(attached)
    deep = copy.deepcopy(attached)

    # Same id, same parent.
    assert shallow.constraint_id == attached.constraint_id
    assert deep.constraint_id == attached.constraint_id
    assert shallow.instance is instance
    assert deep.instance is instance

    # Mutating through the original is visible through both copies.
    attached.set_name("demand")
    assert shallow.name == "demand"
    assert deep.name == "demand"
