"""Kind selectors remain open in type hints and retain runtime value behavior."""

import enum

import pytest
from typing_extensions import assert_type

from ommx import Kind, ProvenanceKind, ConstraintKind


def known_kind_consumer(kind: Kind) -> bool:
    match kind:
        case (
            Kind.Binary
            | Kind.Integer
            | Kind.Continuous
            | Kind.SemiInteger
            | Kind.SemiContinuous
            | Kind.FiniteDomain
        ):
            return True
    # Adding another constant must not change this consumer's type annotation.
    assert_type(kind, Kind)
    raise NotImplementedError(f"Unsupported variable kind: {kind}")


def known_family_consumer(kind: ConstraintKind) -> bool:
    match kind:
        case (
            ConstraintKind.Regular
            | ConstraintKind.Indicator
            | ConstraintKind.OneHot
            | ConstraintKind.Sos1
        ):
            return True
    assert_type(kind, ConstraintKind)
    raise NotImplementedError(f"Unsupported constraint family: {kind}")


def known_provenance_consumer(kind: ProvenanceKind) -> bool:
    match kind:
        case (
            ProvenanceKind.IndicatorConstraint
            | ProvenanceKind.OneHotConstraint
            | ProvenanceKind.Sos1Constraint
        ):
            return True
    assert_type(kind, ProvenanceKind)
    raise NotImplementedError(f"Unsupported provenance family: {kind}")


def test_known_kind_values_and_rejection():
    for code in range(1, 7):
        kind = Kind.from_pb(code)
        assert type(kind) is Kind
        assert known_kind_consumer(kind)
        assert kind.to_pb() == code
        assert int(kind) == code
        assert hash(kind) == hash(code)
        mapping: dict[object, str] = {kind: "value"}
        assert mapping[code] == "value"
        assert not isinstance(kind, enum.Enum)
    for code in [0, -1, 10_000]:
        with pytest.raises(ValueError, match="Invalid kind"):
            Kind.from_pb(code)


def test_special_kind_values():
    for kind in [
        ConstraintKind.Indicator,
        ConstraintKind.OneHot,
        ConstraintKind.Sos1,
    ]:
        assert known_family_consumer(kind)
        assert not isinstance(kind, enum.Enum)


def test_provenance_kind_values():
    assert (
        repr(ProvenanceKind.IndicatorConstraint) == "ProvenanceKind.IndicatorConstraint"
    )
    assert repr(ProvenanceKind.OneHotConstraint) == "ProvenanceKind.OneHotConstraint"
    assert repr(ProvenanceKind.Sos1Constraint) == "ProvenanceKind.Sos1Constraint"
    for kind in [
        ProvenanceKind.IndicatorConstraint,
        ProvenanceKind.OneHotConstraint,
        ProvenanceKind.Sos1Constraint,
    ]:
        assert known_provenance_consumer(kind)
        assert not isinstance(kind, enum.Enum)
