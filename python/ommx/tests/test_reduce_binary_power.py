import sys
import copy

import pytest
from ommx import (
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    IndicatorConstraintID,
    Instance,
    NamedFunction,
    Polynomial,
    Sense,
)


@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
def test_reduction_preserves_indicator_activation(zero, equality):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.binary(1)
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        active_constraints={},
        active_indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=z,
                function=x * x,
                equality=equality,
                trigger_value=int(not zero),
                name="conditional_square",
            )
        },
    )
    reduced = copy.deepcopy(source)
    assert reduced.reduce_binary_power()
    row = reduced.active_constraints[IndicatorConstraintID(7)]
    assert row.trigger_value == int(not zero)
    assert row.equality == equality
    assert row.name == "conditional_square"
    assert row.function.terms == {(1,): 1.0}
    for zv in (0, 1):
        for xv in (0, 1):
            state = {0: zv, 1: xv}
            expected = zv != int(not zero) or xv == 0
            assert source.evaluate(state).feasible == expected
            assert reduced.evaluate(state).feasible_relaxed == expected
    assert not reduced.reduce_binary_power()


def test_reduction_changes_indicator_but_preserves_named_function() -> None:
    x = DecisionVariable.binary(0)
    square = x * x
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        active_constraints={},
        active_indicator_constraints={
            1: IndicatorConstraint(
                indicator_variable=x,
                function=square,
                equality=Equality.EqualToZero,
            )
        },
        named_functions=[NamedFunction(id=2, function=square, name="reported_square")],
    )

    assert instance.reduce_binary_power()
    indicator = instance.active_constraints[IndicatorConstraintID(1)]
    assert indicator.function.terms == {(0,): 1.0}
    assert instance.named_functions[0].function.terms == {(0, 0): 1.0}
    assert instance.named_functions[0].name == "reported_square"
    assert instance.output_objective is None
    assert not instance.reduce_binary_power()


@pytest.mark.parametrize("active_changes", [False, True])
def test_reduction_does_not_inspect_overflowing_named_function(
    active_changes: bool,
) -> None:
    x = DecisionVariable.binary(0)
    huge = sys.float_info.max
    expression = Function(Polynomial({(0,): huge, (0, 0): huge}))
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x * x if active_changes else x,
        decision_variables=[x],
        active_constraints={},
        named_functions=[NamedFunction(id=1, function=expression)],
    )

    assert instance.reduce_binary_power() == active_changes
    assert instance.named_functions[0].function.terms == expression.terms
    if active_changes:
        assert instance.output_objective is not None
        assert instance.output_objective.function.terms == {(0, 0): 1.0}


def test_indicator_overflow_preserves_active_formulation_and_context() -> None:
    x = DecisionVariable.binary(0)
    huge = sys.float_info.max
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x * x,
        decision_variables=[x],
        active_constraints={},
        active_indicator_constraints={
            1: IndicatorConstraint(
                indicator_variable=x,
                function=Function(Polynomial({(0,): huge, (0, 0): huge})),
                equality=Equality.EqualToZero,
            )
        },
        named_functions=[NamedFunction(id=2, function=x * x)],
    )
    before = instance.to_v2_bytes()

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        instance.reduce_binary_power()

    assert instance.to_v2_bytes() == before
