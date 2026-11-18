import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    OneHotConstraint,
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
