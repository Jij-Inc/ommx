"""One typed family selector serves queries and transformations."""

# pyright: reportUnnecessaryTypeIgnoreComment=error

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedSos1Constraint,
    ConstraintIDBase,
    ConstraintKind,
    ConstraintFamilyView,
    ConstraintView,
    Instance,
    Kind,
    ProvenanceKind,
    Sos1ConstraintID,
    SpecialConstraintPreparation,
)
from test_live_constraint_collections import make_model


def generic_family(model: Instance, kind: ConstraintKind[ConstraintIDBase]) -> None:
    assert_type(
        model.active_constraints.of_kind(kind),
        ConstraintFamilyView[ConstraintIDBase, ConstraintView],
    )


def test_unparameterized_kind_returns_common_row():
    def first_row(model: Instance, kind: ConstraintKind) -> ConstraintView:
        rows = model.active_constraints.of_kind(kind)
        assert_type(rows, ConstraintFamilyView[ConstraintIDBase, ConstraintView])
        return assert_type(next(iter(rows.values())), ConstraintView)

    assert isinstance(
        first_row(make_model(Instance), ConstraintKind.Sos1), AttachedSos1Constraint
    )


def test_all_active_families_include_regular():
    model = make_model(Instance)
    assert model.active_constraint_kinds == {
        ConstraintKind.Regular,
        ConstraintKind.Indicator,
        ConstraintKind.OneHot,
        ConstraintKind.Sos1,
    }
    assert model.active_special_constraint_kinds == (
        model.active_constraint_kinds - {ConstraintKind.Regular}
    )
    generic_family(model, ConstraintKind.Regular)


def test_regular_cannot_be_lowered_and_rejection_is_atomic():
    model = make_model(Instance)
    before = model.to_v2_bytes()
    with pytest.raises(RuntimeError, match="Regular constraints cannot be lowered"):
        model.lower_special_constraints({ConstraintKind.OneHot, ConstraintKind.Regular})
    assert model.to_v2_bytes() == before


def test_lowering_accepts_stored_sets_of_known_selectors():
    model = make_model(Instance)
    kinds = {ConstraintKind.Sos1}
    SpecialConstraintPreparation.lower_special_constraints(kinds=kinds)
    assert model.lower_special_constraints(kinds) == kinds

    remaining = frozenset({ConstraintKind.Indicator, ConstraintKind.OneHot})
    SpecialConstraintPreparation.lower_special_constraints(kinds=remaining)
    assert model.lower_special_constraints(remaining) == remaining


def test_kind_constructors_are_unavailable():
    with pytest.raises(TypeError):
        Kind()  # pyright: ignore[reportCallIssue]
    with pytest.raises(TypeError):
        ProvenanceKind()  # pyright: ignore[reportCallIssue]
    with pytest.raises(TypeError):
        ConstraintKind[Sos1ConstraintID]()  # pyright: ignore[reportCallIssue]


def test_strings_are_not_family_selectors():
    model = make_model(Instance)
    with pytest.raises(TypeError, match="ConstraintKind"):
        model.active_constraints.of_kind("sos1")  # type: ignore[call-overload]
    with pytest.raises(TypeError, match="ConstraintKind"):
        model.constraints_df(kind="sos1")  # type: ignore[arg-type]
