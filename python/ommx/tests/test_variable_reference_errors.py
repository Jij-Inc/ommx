from typing import Any

import pytest

from ommx import (
    DecisionVariable,
    InactiveConstraint,
    InactiveIndicatorConstraint,
    InactiveOneHotConstraint,
    InactiveSos1Constraint,
    Instance,
    NamedFunction,
    OneHotConstraint,
    Parameter,
    ParametricInstance,
    Sense,
    Sos1Constraint,
)


def constraint_for(family):
    x, y = DecisionVariable.binary(10), DecisionVariable.binary(20)
    if family == "regular":
        return x + y <= 1
    if family == "indicator":
        return (x + y <= 1).with_indicator(0)
    if family == "one_hot":
        return OneHotConstraint(variables=[x, y])
    return Sos1Constraint(variables=[x, y])


def add_constraint(instance, family, constraint):
    method = "add_constraint" if family == "regular" else f"add_{family}_constraint"
    return getattr(instance, method)(constraint)


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("family", ["regular", "indicator", "one_hot", "sos1"])
@pytest.mark.parametrize("reason", ["Undefined", "Fixed", "Dependent"])
def test_invalid_references_raise_value_error_without_mutation(
    parametric, family, reason
):
    z, x, y = [DecisionVariable.binary(i) for i in [0, 10, 20]]
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=z,
        decision_variables=[z] if reason == "Undefined" else [z, x, y],
        active_constraints={},
    )
    if reason == "Fixed":
        instance = instance.partial_evaluate({10: 0, 20: 1})
    elif reason == "Dependent":
        instance.substitute({10: z, 20: 1 - z})
    target = instance.as_parametric_instance() if parametric else instance
    before = target.to_v2_bytes()

    with pytest.raises(ValueError, match=reason) as caught:
        add_constraint(target, family, constraint_for(family))

    assert "VariableID(10)" in str(caught.value)
    assert "VariableID(20)" in str(caught.value)
    assert target.to_v2_bytes() == before


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize(
    "location", ["objective", "named", "regular", "indicator", "one_hot", "sos1"]
)
def test_from_components_rejects_undefined_references_with_value_error(
    parametric, location
):
    x, y = DecisionVariable.binary(10), DecisionVariable.binary(20)
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[DecisionVariable.binary(0)],
        active_constraints={},
    )
    if location == "objective":
        kwargs["objective"] = x + y
    elif location == "named":
        kwargs["named_functions"] = [NamedFunction(id=1, function=x + y)]
    else:
        key = (
            "active_constraints"
            if location == "regular"
            else f"active_{location}_constraints"
        )
        kwargs[key] = {1: constraint_for(location)}
    if parametric:
        kwargs["parameters"] = [Parameter(100)]
    factory = ParametricInstance if parametric else Instance

    with pytest.raises(ValueError, match="Undefined") as caught:
        factory.from_components(**kwargs)
    assert "VariableID(10)" in str(caught.value)
    assert "VariableID(20)" in str(caught.value)


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize(
    ("family", "wrapper"),
    [
        ("regular", InactiveConstraint),
        ("indicator", InactiveIndicatorConstraint),
        ("one_hot", InactiveOneHotConstraint),
        ("sos1", InactiveSos1Constraint),
    ],
)
def test_inactive_components_also_require_declared_ids(parametric, family, wrapper):
    key = (
        "inactive_constraints"
        if family == "regular"
        else f"inactive_{family}_constraints"
    )
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[DecisionVariable.binary(0)],
        active_constraints={},
    )
    kwargs[key] = {1: wrapper(constraint_for(family), "retained")}
    if parametric:
        kwargs["parameters"] = []
    factory = ParametricInstance if parametric else Instance
    with pytest.raises(ValueError, match="Undefined") as caught:
        factory.from_components(**kwargs)
    assert "VariableID(10)" in str(caught.value)
    assert "VariableID(20)" in str(caught.value)


@pytest.mark.parametrize("family", ["indicator", "one_hot", "sos1"])
def test_parameters_are_rejected_in_structural_positions(family):
    p = Parameter(10)
    target = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=p,
        decision_variables=[DecisionVariable.binary(0)],
        parameters=[p, Parameter(20)],
        active_constraints={},
    )
    # A parameter is valid in a function body.
    target.add_constraint(p <= 1)
    target.add_indicator_constraint((p <= 1).with_indicator(0))
    before = target.to_v2_bytes()
    constraint = (
        (p <= 1).with_indicator(10) if family == "indicator" else constraint_for(family)
    )

    with pytest.raises(ValueError, match="Parameter IDs.*structural"):
        add_constraint(target, family, constraint)
    assert target.to_v2_bytes() == before


@pytest.mark.parametrize("parametric", [False, True])
def test_indicator_builder_reports_unknown_control_and_body_together(parametric):
    x, y = DecisionVariable.binary(10), DecisionVariable.binary(20)
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[],
        active_constraints={},
        active_indicator_constraints={1: (y <= 1).with_indicator(x)},
    )
    if parametric:
        kwargs["parameters"] = []
    factory = ParametricInstance if parametric else Instance
    with pytest.raises(ValueError, match="Undefined") as caught:
        factory.from_components(**kwargs)
    assert "VariableID(10)" in str(caught.value)
    assert "VariableID(20)" in str(caught.value)


def test_objective_rejection_preserves_output_objective():
    instance = Instance.minimize()
    x = instance.new_binary()
    instance.objective = x
    instance.substitute({x.id: 1})
    before = instance.to_v2_bytes()

    with pytest.raises(ValueError, match="Undefined") as caught:
        instance.objective = DecisionVariable.binary(10) + DecisionVariable.binary(20)

    assert "VariableID(10)" in str(caught.value)
    assert "VariableID(20)" in str(caught.value)
    assert instance.to_v2_bytes() == before
