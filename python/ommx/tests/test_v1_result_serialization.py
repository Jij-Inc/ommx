"""v1 export must not discard retained special-constraint results."""

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
)


@pytest.mark.parametrize(
    "family", ["indicator-one", "indicator-zero", "one-hot", "sos1"]
)
@pytest.mark.parametrize("removed", [False, True])
@pytest.mark.parametrize("z_value", [0, 1])
@pytest.mark.parametrize("x_value", [0, 1])
def test_v1_rejects_special_results_and_preserves_v2(family, removed, z_value, x_value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[z, x],
        constraints={},
    )
    if family.startswith("indicator"):
        instance.add_indicator_constraint(
            (x == 0).with_indicator(z, activate_on_zero=family == "indicator-zero")
        )
        expected_family = "Indicator"
        if removed:
            instance.convert_all_indicators_to_constraints()
    elif family == "one-hot":
        instance.add_one_hot_constraint(OneHotConstraint(variables=[0, 1]))
        expected_family = "One-hot"
        if removed:
            instance.convert_all_one_hots_to_constraints()
    else:
        instance.add_sos1_constraint(Sos1Constraint(variables=[0, 1]))
        expected_family = "SOS1"
        if removed:
            instance.convert_all_sos1_to_constraints()

    state = {0: z_value, 1: x_value}
    solution = instance.evaluate(state)
    samples = instance.evaluate_samples({7: state})
    for result in [solution, samples, samples.get(7)]:
        before = result.to_v2_bytes()
        with pytest.raises(RuntimeError, match=expected_family + ".*to_v2_bytes"):
            result.to_v1_bytes()
        assert result.to_v2_bytes() == before
        restored = type(result).from_v2_bytes(before)
        assert restored.to_v2_bytes() == before
        assert restored.feasible == result.feasible


@pytest.mark.parametrize("with_constraint", [False, True])
def test_v1_still_exports_regular_results(with_constraint):
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={0: x == 0} if with_constraint else {},
    )
    solution = instance.evaluate({0: 1})
    samples = instance.evaluate_samples({7: {0: 1}})
    for result in [solution, samples, samples.get(7)]:
        restored = type(result).from_v1_bytes(result.to_v1_bytes())
        assert restored.feasible == result.feasible
        assert restored.to_v1_bytes() == result.to_v1_bytes()
