from __future__ import annotations

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    Sense,
    Sos1BigMPromotion,
    Sos1BigMPromotionRequest,
    Sos1BigMSelectorClaim,
)


def mixed_formulation() -> tuple[Instance, Sos1BigMPromotionRequest]:
    binary_member = DecisionVariable.binary(0)
    integer_member = DecisionVariable.integer(1, lower=-2, upper=3)
    selector = DecisionVariable.binary(10)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[binary_member, integer_member, selector],
        constraints={
            100: integer_member - 3 * selector <= 0,
            101: -integer_member - 2 * selector <= 0,
            102: binary_member + selector - 1 <= 0,
        },
    )
    reused = Sos1BigMSelectorClaim.reused()
    fresh = Sos1BigMSelectorClaim.fresh(
        10,
        upper_link=100,
        lower_link=101,
    )
    request = Sos1BigMPromotionRequest(
        selector_claims={0: reused, 1: fresh},
        cardinality_constraint=102,
    )
    return instance, request


def test_promote_sos1_big_m_exposes_checked_result() -> None:
    instance, request = mixed_formulation()

    assert request.selector_claims[0].is_reused
    assert request.selector_claims[0].selector is None
    fresh = request.selector_claims[1]
    assert not fresh.is_reused
    assert fresh.selector == 10
    assert fresh.upper_link == 100
    assert fresh.lower_link == 101
    assert request.cardinality_constraint == 102

    promotion = instance.promote_sos1_big_m(request)

    assert isinstance(promotion, Sos1BigMPromotion)
    assert promotion.sos1_constraint_id == 0
    assert promotion.members == {0, 1}
    assert promotion.fresh_selectors == {1: 10}
    assert promotion.relaxed_constraint_ids == {100, 101, 102}
    assert instance.constraints == {}
    assert set(instance.removed_constraints) == {100, 101, 102}
    assert set(instance.sos1_constraints) == {0}
    assert instance.populate_state({0: 0, 1: 2}).entries[10] == 1


def test_promote_sos1_big_m_rejects_invalid_request_atomically() -> None:
    instance, valid_request = mixed_formulation()
    invalid_request = Sos1BigMPromotionRequest(
        selector_claims=valid_request.selector_claims,
        cardinality_constraint=999,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(RuntimeError, match="is not active"):
        instance.promote_sos1_big_m(invalid_request)

    assert instance.to_v2_bytes() == before
