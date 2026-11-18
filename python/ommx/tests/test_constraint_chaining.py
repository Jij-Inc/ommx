import copy

import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    ParametricInstance,
    Sense,
    Sos1Constraint,
)


@pytest.fixture(
    params=[Constraint, IndicatorConstraint, OneHotConstraint, Sos1Constraint]
)
def constraint(request):
    cls = request.param
    if cls is Constraint:
        return DecisionVariable.binary(0) <= 1
    if cls is IndicatorConstraint:
        return cls(
            indicator_variable=0,
            function=DecisionVariable.binary(1) - 1,
            equality=Equality.LessThanOrEqualToZero,
        )
    return cls(variables=[0, 1])


@pytest.mark.parametrize(
    ("method", "value", "attribute"),
    [
        ("set_name", "limit", "name"),
        ("set_subscripts", [1, 2], "subscripts"),
        ("set_description", "A limit", "description"),
        ("set_parameters", {"group": "a"}, "parameters"),
    ],
)
def test_setter_returns_same_constraint(constraint, method, value, attribute):
    assert getattr(constraint, method)(value) is constraint
    assert getattr(constraint, attribute) == value


def test_chaining_updates_original_constraint(constraint):
    constraint.set_name("limit").set_subscripts([1]).set_description(
        "A limit"
    ).set_parameters({"group": "a"})

    assert constraint.name == "limit"
    assert constraint.subscripts == [1]
    assert constraint.description == "A limit"
    assert constraint.parameters == {"group": "a"}


@pytest.mark.parametrize("copy_constraint", [copy.copy, copy.deepcopy])
def test_chaining_on_copy_preserves_independent_metadata(constraint, copy_constraint):
    constraint.set_name("original").set_subscripts([0]).set_parameters({"group": "a"})
    cloned = copy_constraint(constraint)

    assert cloned is not constraint
    assert (
        cloned.set_name("copy").set_subscripts([1]).set_parameters({"group": "b"})
        is cloned
    )
    assert (cloned.name, cloned.subscripts, cloned.parameters) == (
        "copy",
        [1],
        {"group": "b"},
    )
    assert (constraint.name, constraint.subscripts, constraint.parameters) == (
        "original",
        [0],
        {"group": "a"},
    )


@pytest.mark.parametrize("host_type", [Instance, ParametricInstance])
def test_standalone_chaining_does_not_modify_registered_constraint(
    constraint, host_type
):
    variables = [DecisionVariable.binary(0), DecisionVariable.binary(1)]
    if host_type is ParametricInstance:
        model = ParametricInstance.from_components(
            sense=Sense.Minimize,
            objective=0,
            decision_variables=variables,
            active_constraints={},
            parameters=[],
        )
    else:
        model = Instance.from_components(
            sense=Sense.Minimize,
            objective=0,
            decision_variables=variables,
            active_constraints={},
        )
    insert = {
        Constraint: model.add_constraint,
        IndicatorConstraint: model.add_indicator_constraint,
        OneHotConstraint: model.add_one_hot_constraint,
        Sos1Constraint: model.add_sos1_constraint,
    }[type(constraint)]
    constraint.set_name("registered").set_subscripts([0])
    attached = insert(constraint)

    constraint.set_name("standalone").set_subscripts([1])
    snapshot = attached.detach()
    assert snapshot.set_name("snapshot").set_subscripts([2]) is snapshot

    assert (constraint.name, constraint.subscripts) == ("standalone", [1])
    assert (snapshot.name, snapshot.subscripts) == ("snapshot", [2])
    assert (attached.name, attached.subscripts) == ("registered", [0])


def test_constraint_append_methods_preserve_identity_and_metadata():
    constraint = (DecisionVariable.binary(0) <= 1).set_subscripts([0])
    constraint.set_parameters({"keep": "yes", "replace": "old"})

    assert constraint.add_subscripts([1]) is constraint
    assert constraint.add_parameters({"replace": "new"}) is constraint
    assert constraint.add_parameter("extra", "value") is constraint
    constraint.add_subscripts([2]).add_parameters({"last": "value"})

    assert constraint.subscripts == [0, 1, 2]
    assert constraint.parameters == {
        "keep": "yes",
        "replace": "new",
        "extra": "value",
        "last": "value",
    }


def test_partial_evaluate_returns_same_constraint_and_can_be_chained():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    constraint = x + y <= 1

    assert constraint.partial_evaluate({0: 1}) is constraint
    constraint.partial_evaluate({1: 0}).set_name("evaluated")

    assert constraint.name == "evaluated"
    assert constraint.evaluate({}).evaluated_value == 0


def test_partial_evaluate_error_releases_borrow_and_allows_retry():
    constraint = DecisionVariable.binary(0) <= 1
    with pytest.raises(ValueError):
        constraint.partial_evaluate({0: 0}, atol=-1)

    assert constraint.evaluate({0: 1}).evaluated_value == 0
    assert constraint.partial_evaluate({0: 0}).set_name("retried") is constraint
    assert constraint.name == "retried"
    assert constraint.evaluate({}).evaluated_value == -1
