from __future__ import annotations

import pytest
from ommx import (
    AdditionalCapability,
    DecisionVariable,
    Instance,
    OneHotConstraint,
    OneHotPromotionCertificate,
    PromotionAudit,
    PromotionPreview,
    PromotionReport,
    PromotionResult,
)


def _instance_with_one_source() -> Instance:
    x = [DecisionVariable.binary(i) for i in range(3)]
    source = (2 * x[0] + 2 * x[1] + 2 * x[2] == 2).set_name("choose")
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={10: source},
        sense=Instance.MINIMIZE,
    )


def _certificate(
    source_constraint_id: int = 10,
    variables: list[int] | None = None,
    target_one_hot_constraint_id: int | None = None,
) -> OneHotPromotionCertificate:
    return OneHotPromotionCertificate(
        source_constraint_id=source_constraint_id,
        variables=[0, 1, 2] if variables is None else variables,
        target_one_hot_constraint_id=target_one_hot_constraint_id,
    )


def test_certificate_shape_and_dry_run() -> None:
    with pytest.raises(ValueError, match="must be unique"):
        _certificate(variables=[0, 0, 1])

    assert repr(_certificate(target_one_hot_constraint_id=7)) == (
        "OneHotPromotionCertificate(source_constraint_id=10, variables=[0, 1, 2], "
        "target_one_hot_constraint_id=7)"
    )
    assert repr(_certificate()).endswith("target_one_hot_constraint_id=None)")

    empty = _certificate(variables=[])
    instance = _instance_with_one_source()
    with pytest.raises(RuntimeError, match="must not be empty"):
        instance.check_promotion_certificate(
            empty, allowed={AdditionalCapability.OneHot}
        )

    certificate = _certificate()
    before = instance.to_v2_bytes()
    preview = instance.check_promotion_certificate(
        certificate, allowed={AdditionalCapability.OneHot}
    )
    assert isinstance(preview, PromotionPreview)
    assert preview.source_constraint_id == 10
    assert preview.variables == [0, 1, 2]
    assert preview.target_one_hot_constraint_id == 0
    assert instance.to_v2_bytes() == before

    with pytest.raises(RuntimeError, match="allowed capabilities"):
        instance.check_promotion_certificate(certificate, allowed=set())


def test_single_promotion_and_audit_round_trip() -> None:
    instance = _instance_with_one_source()
    result = instance.promote_with_certificate(
        _certificate(), allowed={AdditionalCapability.OneHot}
    )

    assert isinstance(result, PromotionResult)
    assert result.source_constraint_id == 10
    assert result.target_one_hot_constraint_id == 0
    assert instance.required_capabilities == {AdditionalCapability.OneHot}
    assert list(instance.constraints) == []
    assert instance.one_hot_constraints[0].variables == [0, 1, 2]
    assert instance.one_hot_constraints[0].name == "choose"

    removed = instance.removed_constraints[10]
    assert removed.removed_reason == "ommx.Instance.promote_constraint_to_one_hot"
    assert removed.removed_reason_parameters == {
        "promotion.kind": "one_hot",
        "promotion.target_id": "0",
        "promotion.certificate_version": "1",
    }

    audit = instance.verify_promotion_history(10)
    assert isinstance(audit, PromotionAudit)
    assert audit.source_constraint_id == 10
    assert audit.variables == [0, 1, 2]
    assert audit.target_one_hot_constraint_id == 0
    assert audit.target_is_active is True

    round_trip = Instance.from_v2_bytes(instance.to_v2_bytes())
    assert round_trip.verify_promotion_history(10) == audit


def test_bulk_reserves_explicit_targets_and_is_atomic() -> None:
    x = [DecisionVariable.binary(i) for i in range(4)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={
            10: x[0] + x[1] == 1,
            11: -2 * x[2] - 2 * x[3] == -2,
        },
        one_hot_constraints={5: OneHotConstraint(variables=[0])},
        sense=Instance.MINIMIZE,
    )
    report = instance.promote_with_certificates(
        [
            _certificate(10, [0, 1]),
            _certificate(11, [2, 3], target_one_hot_constraint_id=10),
        ],
        allowed={AdditionalCapability.OneHot},
    )
    assert isinstance(report, PromotionReport)
    assert report.source_to_target == {10: 11, 11: 10}

    invalid = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={
            20: x[0] + x[1] == 1,
            21: x[2] + x[3] <= 1,
        },
        sense=Instance.MINIMIZE,
    )
    before = invalid.to_v2_bytes()
    with pytest.raises(RuntimeError, match="not an equality"):
        invalid.promote_with_certificates(
            [_certificate(20, [0, 1]), _certificate(21, [2, 3])],
            allowed={AdditionalCapability.OneHot},
        )
    assert invalid.to_v2_bytes() == before


def test_lowering_blocks_restore_of_promoted_source() -> None:
    instance = _instance_with_one_source()
    instance.promote_with_certificate(
        _certificate(), allowed={AdditionalCapability.OneHot}
    )
    assert instance.reduce_capabilities(set()) == {AdditionalCapability.OneHot}
    assert instance.required_capabilities == set()
    assert 0 in instance.removed_one_hot_constraints

    audit = instance.verify_promotion_history(10)
    assert audit.target_is_active is False
    before = instance.to_v2_bytes()
    with pytest.raises(RuntimeError, match="Cannot restore promoted"):
        instance.restore_constraint(10)
    assert instance.to_v2_bytes() == before
