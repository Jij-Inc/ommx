from __future__ import annotations

import pytest

from ommx import (
    DecisionVariable,
    Instance,
    Sense,
    Sos1BigMPromotion,
    Sos1BigMPromotionBatchRejectedError,
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

    promotions = instance.promote_sos1_big_m([request])

    assert len(promotions) == 1
    promotion = promotions[0]
    assert isinstance(promotion, Sos1BigMPromotion)
    assert promotion.sos1_constraint_id == 0
    assert promotion.members == {0, 1}
    assert promotion.fresh_selectors == {1: 10}
    assert promotion.relaxed_constraint_ids == {100, 101, 102}
    assert instance.constraints == {}
    assert set(instance.removed_constraints) == {100, 101, 102}
    assert set(instance.sos1_constraints) == {0}
    assert instance.populate_state({0: 0, 1: 2}).entries[10] == 1


def test_promote_sos1_big_m_accepts_tight_continuous_links() -> None:
    member = DecisionVariable.continuous(1, lower=-2, upper=2)
    selector = DecisionVariable.binary(10)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[member, selector],
        constraints={
            100: member - 2 * selector <= 0,
            101: -member - 2 * selector <= 0,
            102: selector - 1 <= 0,
        },
    )
    request = Sos1BigMPromotionRequest(
        selector_claims={
            1: Sos1BigMSelectorClaim.fresh(
                10,
                upper_link=100,
                lower_link=101,
            )
        },
        cardinality_constraint=102,
    )

    [promotion] = instance.promote_sos1_big_m([request], atol=1e-6)

    assert promotion.members == {1}
    assert promotion.fresh_selectors == {1: 10}


def test_promote_sos1_big_m_applies_a_fully_valid_batch_in_order() -> None:
    first = DecisionVariable.binary(0)
    second = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[first, second],
        constraints={
            100: first - 1 <= 0,
            101: first + second - 1 <= 0,
        },
    )
    requests = [
        Sos1BigMPromotionRequest(
            selector_claims={0: Sos1BigMSelectorClaim.reused()},
            cardinality_constraint=100,
        ),
        Sos1BigMPromotionRequest(
            selector_claims={
                0: Sos1BigMSelectorClaim.reused(),
                1: Sos1BigMSelectorClaim.reused(),
            },
            cardinality_constraint=101,
        ),
    ]

    promotions = instance.promote_sos1_big_m(requests)

    assert [promotion.sos1_constraint_id for promotion in promotions] == [0, 1]
    assert [promotion.members for promotion in promotions] == [{0}, {0, 1}]
    assert set(instance.removed_constraints) == {100, 101}
    assert set(instance.sos1_constraints) == {0, 1}


def test_promote_sos1_big_m_rejects_the_full_batch_atomically() -> None:
    instance, valid_request = mixed_formulation()
    invalid_request = Sos1BigMPromotionRequest(
        selector_claims=valid_request.selector_claims,
        cardinality_constraint=999,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(
        Sos1BigMPromotionBatchRejectedError,
        match="is not active",
    ) as exc_info:
        instance.promote_sos1_big_m([invalid_request, valid_request])

    assert isinstance(exc_info.value, RuntimeError)
    assert exc_info.value.request_count == 2
    assert set(exc_info.value.rejections) == {0}
    assert "is not active" in exc_info.value.rejections[0]
    assert instance.to_v2_bytes() == before


def test_promote_sos1_big_m_reports_every_conflicting_request() -> None:
    instance, request = mixed_formulation()
    before = instance.to_v2_bytes()

    with pytest.raises(Sos1BigMPromotionBatchRejectedError) as exc_info:
        instance.promote_sos1_big_m([request, request])

    assert exc_info.value.request_count == 2
    assert set(exc_info.value.rejections) == {0, 1}
    assert all(
        "conflicts with another individually valid request" in message
        for message in exc_info.value.rejections.values()
    )
    assert instance.to_v2_bytes() == before


def test_promote_sos1_big_m_accepts_an_empty_batch() -> None:
    instance, _ = mixed_formulation()
    before = instance.to_v2_bytes()

    assert instance.promote_sos1_big_m([], atol=float("inf")) == []

    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize(
    ("atol", "error_type"),
    [
        (0.0, ValueError),
        (1.0, Sos1BigMPromotionBatchRejectedError),
        (float("inf"), Sos1BigMPromotionBatchRejectedError),
    ],
)
def test_promote_sos1_big_m_rejects_invalid_atol_atomically(
    atol: float,
    error_type: type[Exception],
) -> None:
    instance, request = mixed_formulation()
    before = instance.to_v2_bytes()

    with pytest.raises(error_type):
        instance.promote_sos1_big_m([request], atol=atol)

    assert instance.to_v2_bytes() == before
