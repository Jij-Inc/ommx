"""Tests for AttachedIndicatorConstraint write-through wrapper."""

import copy

import pytest

from ommx.v1 import (
    AttachedIndicatorConstraint,
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
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
        decision_variables=[
            DecisionVariable.binary(0),
            DecisionVariable.binary(1),  # indicator variable
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


def _make_indicator(name: str | None = "balance") -> IndicatorConstraint:
    indicator_var = DecisionVariable.binary(1)
    linear = Linear({0: 1.0, 2: 1.0}, -1.0)
    return IndicatorConstraint(
        indicator_variable=indicator_var,
        function=Function.from_linear(linear),
        equality=Equality.EqualToZero,
        name=name,
        subscripts=[7],
        description="initial",
        parameters={"k": "v"},
    )


def test_add_returns_attached_with_drained_metadata():
    """add_indicator_constraint returns AttachedIndicatorConstraint reading
    the staged metadata."""
    instance = _empty_instance()
    snapshot = _make_indicator(name="balance")

    attached = instance.add_indicator_constraint(snapshot)

    assert isinstance(attached, AttachedIndicatorConstraint)
    assert attached.name == "balance"
    assert attached.subscripts == [7]
    assert attached.description == "initial"
    assert attached.parameters == {"k": "v"}
    assert attached.indicator_variable_id == 1
    assert attached.equality == Equality.EqualToZero


def test_setter_writes_through_to_instance():
    """Mutations on AttachedIndicatorConstraint land in the parent instance."""
    instance = _empty_instance()
    attached = instance.add_indicator_constraint(_make_indicator(name="balance"))
    cid = attached.constraint_id

    attached.set_name("demand")
    attached.set_subscripts([1, 2])
    attached.set_description("updated")
    attached.set_parameters({"a": "1"})

    fresh = instance.indicator_constraints[cid]
    assert fresh.name == "demand"
    assert fresh.subscripts == [1, 2]
    assert fresh.description == "updated"
    assert fresh.parameters == {"a": "1"}


def test_two_handles_share_state():
    """Two AttachedIndicatorConstraint handles for the same id observe the
    same data."""
    instance = _empty_instance()
    a = instance.add_indicator_constraint(_make_indicator(name="balance"))
    b = instance.indicator_constraints[a.constraint_id]

    a.set_name("demand")
    assert b.name == "demand"


def test_indicator_constraints_getter_returns_attached():
    """instance.indicator_constraints values are AttachedIndicatorConstraint."""
    instance = _empty_instance()
    a = instance.add_indicator_constraint(_make_indicator(name="c1"))
    b = instance.add_indicator_constraint(_make_indicator(name="c2"))

    constraints = instance.indicator_constraints
    assert set(constraints.keys()) == {a.constraint_id, b.constraint_id}
    assert all(isinstance(c, AttachedIndicatorConstraint) for c in constraints.values())


def test_detach_returns_independent_snapshot():
    """detach() returns an IndicatorConstraint snapshot whose mutations do
    not propagate back."""
    instance = _empty_instance()
    attached = instance.add_indicator_constraint(_make_indicator(name="balance"))

    snapshot = attached.detach()
    assert isinstance(snapshot, IndicatorConstraint)
    assert snapshot.name == "balance"


def test_attached_keeps_instance_alive_after_del():
    """The handle keeps the parent instance alive across `del instance`."""
    instance = _empty_instance()
    attached = instance.add_indicator_constraint(_make_indicator(name="balance"))

    del instance

    assert attached.name == "balance"
    attached.set_name("demand")
    assert attached.name == "demand"
    snapshot = attached.detach()
    assert snapshot.name == "demand"


def test_add_rejects_undefined_indicator_variable():
    """add_indicator_constraint validates that all referenced ids are
    defined."""
    instance = _empty_instance()  # variables = [0, 1, 2]
    rogue_indicator = DecisionVariable.binary(99)  # not registered
    bad = IndicatorConstraint(
        indicator_variable=rogue_indicator,
        function=Function.from_linear(Linear({0: 1.0}, 0.0)),
        equality=Equality.EqualToZero,
    )

    with pytest.raises(Exception, match="99"):
        instance.add_indicator_constraint(bad)

    assert instance.indicator_constraints == {}


def test_copy_and_deepcopy_share_parent_instance():
    """copy / deepcopy produce another handle pointing at the same id on
    the same parent."""
    instance = _empty_instance()
    attached = instance.add_indicator_constraint(_make_indicator(name="balance"))

    shallow = copy.copy(attached)
    deep = copy.deepcopy(attached)

    assert shallow.constraint_id == attached.constraint_id
    assert deep.constraint_id == attached.constraint_id
    assert shallow.instance is instance
    assert deep.instance is instance

    attached.set_name("demand")
    assert shallow.name == "demand"
    assert deep.name == "demand"


# --- ParametricInstance host ---


def test_add_returns_attached_on_parametric_host():
    """ParametricInstance.add_indicator_constraint mirrors Instance."""
    parametric = _empty_parametric_instance()
    snapshot = _make_indicator(name="balance")

    attached = parametric.add_indicator_constraint(snapshot)

    assert isinstance(attached, AttachedIndicatorConstraint)
    assert attached.instance is parametric
    assert attached.name == "balance"


def test_setter_writes_through_on_parametric_host():
    """Write-through works for indicator constraints on parametric hosts."""
    parametric = _empty_parametric_instance()
    attached = parametric.add_indicator_constraint(_make_indicator(name="balance"))
    cid = attached.constraint_id

    attached.set_name("demand")
    attached.set_subscripts([1, 2, 3])

    fresh = parametric.indicator_constraints[cid]
    assert fresh.name == "demand"
    assert fresh.subscripts == [1, 2, 3]


def test_add_indicator_accepts_parameter_in_function_on_parametric():
    """The function may reference a parameter id on a parametric host (the
    indicator variable itself must still be a real decision variable)."""
    parametric = _empty_parametric_instance()
    indicator_var = DecisionVariable.binary(1)
    constraint = IndicatorConstraint(
        indicator_variable=indicator_var,
        function=Function.from_linear(Linear({0: 1.0, 100: -1.0}, 0.0)),
        equality=Equality.EqualToZero,
        name="param_link",
    )

    attached = parametric.add_indicator_constraint(constraint)
    assert attached.name == "param_link"


def test_add_indicator_rejects_parameter_in_indicator_variable_position():
    """The indicator variable is a structural position — substitution can't
    fill it later, so a parameter id in that slot must be rejected on a
    parametric host."""
    parametric = _empty_parametric_instance()  # parameters = [100]
    # parameter id 100 used as the indicator variable; this is invalid.
    rogue_indicator = DecisionVariable.binary(100)
    bad = IndicatorConstraint(
        indicator_variable=rogue_indicator,
        function=Function.from_linear(Linear({0: 1.0}, 0.0)),
        equality=Equality.EqualToZero,
    )

    with pytest.raises(Exception, match="(structural|parameter)"):
        parametric.add_indicator_constraint(bad)

    assert parametric.indicator_constraints == {}
