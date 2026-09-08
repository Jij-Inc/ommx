from __future__ import annotations

from typing import Literal

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

Mode = Literal["best_effort", "strict"]


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
    request = Sos1BigMPromotionRequest(
        selector_claims={
            102: {
                0: Sos1BigMSelectorClaim.reused(),
                1: Sos1BigMSelectorClaim.fresh(10, upper_link=100, lower_link=101),
            }
        },
    )
    return instance, request


def assert_report_keys(
    report: Sos1BigMPromotion, request: Sos1BigMPromotionRequest
) -> None:
    assert isinstance(report, Sos1BigMPromotion)
    assert report.request_count == len(request.selector_claims)
    assert set(report.promoted).isdisjoint(report.rejections)
    assert set(report.promoted) | set(report.rejections) == set(request.selector_claims)


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
def test_promote_sos1_big_m_exposes_batch_report(mode: Mode) -> None:
    instance, request = mixed_formulation()

    claims = request.selector_claims[102]
    assert claims[0].is_reused
    assert claims[0].selector is None
    fresh = claims[1]
    assert not fresh.is_reused
    assert fresh.selector == 10
    assert fresh.upper_link == 100
    assert fresh.lower_link == 101

    report = instance.promote_sos1_big_m(request, mode=mode)

    assert_report_keys(report, request)
    assert report.promoted == {102: 0}
    assert report.rejections == {}
    assert set(instance.sos1_constraints[0].variables) == {0, 1}
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
            102: {1: Sos1BigMSelectorClaim.fresh(10, upper_link=100, lower_link=101)}
        },
    )

    report = instance.promote_sos1_big_m(request, atol=1e-6)

    assert_report_keys(report, request)
    assert report.promoted == {102: 0}
    assert set(instance.sos1_constraints[0].variables) == {1}


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
def test_promote_sos1_big_m_keys_shared_member_formulations_by_cardinality(
    mode: Mode,
) -> None:
    first = DecisionVariable.binary(0)
    second = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[first, second],
        constraints={0: first - 1 <= 0, 2**64 - 1: first + second - 1 <= 0},
    )
    # Reverse insertion order and boundary IDs distinguish keys from positions.
    request = Sos1BigMPromotionRequest(
        selector_claims={
            2**64 - 1: {
                0: Sos1BigMSelectorClaim.reused(),
                1: Sos1BigMSelectorClaim.reused(),
            },
            0: {0: Sos1BigMSelectorClaim.reused()},
        },
    )

    report = instance.promote_sos1_big_m(request, mode=mode)

    assert_report_keys(report, request)
    assert report.promoted == {0: 0, 2**64 - 1: 1}
    assert report.rejections == {}
    assert set(instance.sos1_constraints[0].variables) == {0}
    assert set(instance.sos1_constraints[1].variables) == {0, 1}
    assert set(instance.removed_constraints) == {0, 2**64 - 1}
    assert set(instance.sos1_constraints) == {0, 1}


def test_promote_sos1_big_m_defaults_to_best_effort_and_retains_all_rejections() -> (
    None
):
    instance, valid_request = mixed_formulation()
    claims = valid_request.selector_claims
    claims[999] = claims[102]
    claims[2**64 - 1] = {}
    request = Sos1BigMPromotionRequest(claims)

    report = instance.promote_sos1_big_m(request)

    assert_report_keys(report, request)
    assert report.promoted == {102: 0}
    assert set(report.rejections) == {999, 2**64 - 1}
    assert "is not active" in report.rejections[999]
    assert "at least one member" in report.rejections[2**64 - 1]
    assert instance.constraints == {}
    assert set(instance.removed_constraints) == {100, 101, 102}
    assert set(instance.sos1_constraints) == {0}
    assert instance.populate_state({0: 0, 1: 2}).entries[10] == 1

    # Derived snapshots cannot change the report's exclusive per-key outcomes.
    report.promoted.clear()
    report.rejections.clear()
    assert_report_keys(report, request)
    assert report.promoted == {102: 0}
    assert len(report.rejections) == 2


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
def test_promote_sos1_big_m_rejects_overlapping_links(mode: Mode) -> None:
    member = DecisionVariable.integer(1, lower=0, upper=3)
    selector = DecisionVariable.binary(10)
    independent_member = DecisionVariable.binary(2)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[member, selector, independent_member],
        constraints={
            100: member - 3 * selector <= 0,
            102: selector - 1 <= 0,
            103: selector - 1 <= 0,
            202: independent_member - 1 <= 0,
        },
    )
    overlapping = {1: Sos1BigMSelectorClaim.fresh(10, upper_link=100)}
    request = Sos1BigMPromotionRequest(
        {102: overlapping, 103: overlapping, 202: {2: Sos1BigMSelectorClaim.reused()}}
    )
    before = instance.to_v2_bytes()

    if mode == "strict":
        with pytest.raises(Sos1BigMPromotionBatchRejectedError) as exc_info:
            instance.promote_sos1_big_m(request, mode=mode)
        assert exc_info.value.request_count == 3
        assert set(exc_info.value.rejections) == {102, 103}
        assert instance.to_v2_bytes() == before
    else:
        report = instance.promote_sos1_big_m(request, mode=mode)
        assert_report_keys(report, request)
        assert report.promoted == {202: 0}
        assert set(report.rejections) == {102, 103}
        assert all(
            "outside the claimed formulation" in reason
            for reason in report.rejections.values()
        )
        assert set(instance.constraints) == {100, 102, 103}
        assert set(instance.removed_constraints) == {202}
        assert set(instance.sos1_constraints) == {0}


def test_promote_sos1_big_m_strict_rejects_the_full_batch_atomically() -> None:
    instance, valid_request = mixed_formulation()
    claims = valid_request.selector_claims
    claims[999] = claims[102]
    claims[2**64 - 1] = {}
    request = Sos1BigMPromotionRequest(claims)
    before = instance.to_v2_bytes()

    with pytest.raises(Sos1BigMPromotionBatchRejectedError) as exc_info:
        instance.promote_sos1_big_m(request, mode="strict")

    assert isinstance(exc_info.value, RuntimeError)
    assert exc_info.value.request_count == 3
    assert set(exc_info.value.rejections) == {999, 2**64 - 1}
    assert "is not active" in exc_info.value.rejections[999]
    assert "at least one member" in exc_info.value.rejections[2**64 - 1]
    assert instance.to_v2_bytes() == before

    report = instance.promote_sos1_big_m(valid_request, mode="strict")
    assert_report_keys(report, valid_request)
    assert report.promoted == {102: 0}
    assert report.rejections == {}


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
def test_promote_sos1_big_m_accepts_an_empty_batch(mode: Mode) -> None:
    instance, _ = mixed_formulation()
    before = instance.to_v2_bytes()
    request = Sos1BigMPromotionRequest({})

    report = instance.promote_sos1_big_m(request, mode=mode, atol=float("inf"))

    assert_report_keys(report, request)
    assert report.request_count == 0
    assert report.promoted == {}
    assert report.rejections == {}
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
@pytest.mark.parametrize("atol", [1.0, float("inf")])
def test_promote_sos1_big_m_rejects_unsupported_atol(mode: Mode, atol: float) -> None:
    instance, request = mixed_formulation()
    before = instance.to_v2_bytes()

    if mode == "strict":
        with pytest.raises(Sos1BigMPromotionBatchRejectedError) as exc_info:
            instance.promote_sos1_big_m(request, mode=mode, atol=atol)
        assert set(exc_info.value.rejections) == {102}
    else:
        report = instance.promote_sos1_big_m(request, mode=mode, atol=atol)
        assert_report_keys(report, request)
        assert report.promoted == {}
        assert set(report.rejections) == {102}

    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("mode", ["best_effort", "strict"])
@pytest.mark.parametrize("atol", [0.0, -1.0, float("nan")])
@pytest.mark.parametrize("empty", [False, True])
def test_promote_sos1_big_m_raises_for_malformed_atol(
    mode: Mode, atol: float, empty: bool
) -> None:
    instance, request = mixed_formulation()
    before = instance.to_v2_bytes()

    with pytest.raises(ValueError):
        instance.promote_sos1_big_m(
            Sos1BigMPromotionRequest({}) if empty else request, mode=mode, atol=atol
        )

    assert instance.to_v2_bytes() == before


def test_promote_sos1_big_m_rejects_unknown_mode_before_mutation() -> None:
    instance, request = mixed_formulation()
    before = instance.to_v2_bytes()

    with pytest.raises(ValueError, match="Unknown SOS1 promotion mode"):
        instance.promote_sos1_big_m(request, mode="typo")  # type: ignore[arg-type]

    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("attribute", ["request_count", "rejections"])
def test_strict_rejection_preserves_python_attribute_errors(
    monkeypatch: pytest.MonkeyPatch, attribute: str
) -> None:
    instance, _ = mixed_formulation()
    before = instance.to_v2_bytes()
    failure = AttributeError("custom rejection descriptor failed")

    def reject_assignment(_self: object, _value: object) -> None:
        raise failure

    # Exception classes are mutable Python types. Attribute attachment is not
    # an internal Rust invariant: a user descriptor can reject it.
    monkeypatch.setattr(
        Sos1BigMPromotionBatchRejectedError,
        attribute,
        property(fset=reject_assignment),
        raising=False,
    )
    with pytest.raises(AttributeError) as exc_info:
        instance.promote_sos1_big_m(Sos1BigMPromotionRequest({999: {}}), mode="strict")

    assert exc_info.value is failure
    assert instance.to_v2_bytes() == before
