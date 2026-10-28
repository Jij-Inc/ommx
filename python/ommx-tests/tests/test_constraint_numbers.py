from itertools import combinations

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    OneHotConstraint,
    ParametricInstance,
    Sense,
    Sos1Constraint,
    Sos1BigMPromotionRequest,
    Sos1BigMSelectorClaim,
)


@pytest.mark.parametrize("parametric", [False, True])
@pytest.mark.parametrize("families", list(combinations(range(4), 2)))
def test_model_constructor_rejects_cross_family_numbers(
    parametric: bool, families: tuple[int, int]
):
    x = DecisionVariable.binary(0)
    regular = {42: x == 0} if 0 in families else {}
    indicator = {42: (x == 0).with_indicator(x)} if 1 in families else {}
    one_hot = {42: OneHotConstraint(variables=[x])} if 2 in families else {}
    sos1 = {42: Sos1Constraint(variables=[x])} if 3 in families else {}
    with pytest.raises(RuntimeError, match="Constraint number 42"):
        if parametric:
            ParametricInstance.from_components(
                sense=Sense.Minimize,
                objective=0,
                decision_variables=[x],
                parameters=[],
                constraints=regular,
                indicator_constraints=indicator,
                one_hot_constraints=one_hot,
                sos1_constraints=sos1,
            )
        else:
            Instance.from_components(
                sense=Sense.Minimize,
                objective=0,
                decision_variables=[x],
                constraints=regular,
                indicator_constraints=indicator,
                one_hot_constraints=one_hot,
                sos1_constraints=sos1,
            )


@pytest.mark.parametrize("parametric", [False, True])
def test_added_constraints_share_numbers_across_families(parametric):
    x = DecisionVariable.binary(0)
    instance: Instance | ParametricInstance = Instance.from_components(
        sense=Sense.Minimize, objective=0, decision_variables=[x], constraints={}
    )
    if parametric:
        instance = ParametricInstance.from_components(
            sense=Sense.Minimize,
            objective=0,
            decision_variables=[x],
            constraints={},
            parameters=[],
        )
    assert instance.add_constraint(x == 0).constraint_id == 0
    assert (
        instance.add_indicator_constraint((x == 0).with_indicator(x)).constraint_id == 1
    )
    assert (
        instance.add_one_hot_constraint(OneHotConstraint(variables=[x])).constraint_id
        == 2
    )
    assert (
        instance.add_sos1_constraint(Sos1Constraint(variables=[x])).constraint_id == 3
    )
    restored = type(instance).from_v2_bytes(instance.to_v2_bytes())
    assert restored.to_v2_bytes() == instance.to_v2_bytes()


def test_promotion_rejects_exhausted_shared_number_space_atomically():
    x = DecisionVariable.binary(0)
    maximum = 2**64 - 1
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[x],
        constraints={maximum: x <= 1},
    )
    before = instance.to_v2_bytes()
    request = Sos1BigMPromotionRequest({maximum: {0: Sos1BigMSelectorClaim.reused()}})
    report = instance.promote_sos1_big_m(request)
    assert report.promoted == {}
    assert "Cannot allocate" in report.rejections[maximum]
    assert instance.to_v2_bytes() == before
