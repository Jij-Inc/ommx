"""Generated constraint ID exhaustion is a normal SDK error, not a panic."""

from ommx import ConstraintID, IndicatorConstraintID, OneHotConstraintID

import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    Sense,
    SpecialConstraintKind,
)


@pytest.mark.parametrize("family", ["one_hot", "indicator"])
@pytest.mark.parametrize("entrypoint", ["single", "bulk", "lower"])
def test_lowering_id_exhaustion_returns_runtime_error_without_mutation(
    family: str, entrypoint: str
):
    x = DecisionVariable.continuous(0, lower=-1, upper=1)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints={
            ConstraintID(2**64 - 1): Constraint(
                function=x, equality=Equality.EqualToZero
            )
        },
        one_hot_constraints={OneHotConstraintID(7): OneHotConstraint(variables=[y])}
        if family == "one_hot"
        else {},
        indicator_constraints={
            IndicatorConstraintID(7): IndicatorConstraint(
                indicator_variable=y, function=x, equality=Equality.EqualToZero
            )
        }
        if family == "indicator"
        else {},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(RuntimeError, match="Cannot allocate"):
        if entrypoint == "lower":
            kind = (
                SpecialConstraintKind.OneHot
                if family == "one_hot"
                else SpecialConstraintKind.Indicator
            )
            instance.lower_special_constraints({kind})
        elif family == "one_hot":
            if entrypoint == "single":
                instance.convert_one_hot_to_constraint(OneHotConstraintID(7))
            else:
                instance.convert_all_one_hots_to_constraints()
        elif entrypoint == "single":
            instance.convert_indicator_to_constraint(IndicatorConstraintID(7))
        else:
            instance.convert_all_indicators_to_constraints()

    assert instance.to_v2_bytes() == before
