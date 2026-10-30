"""One typed family selector serves queries and transformations."""

import pytest
from typing_extensions import assert_type

from ommx import (
    ConstraintIDBase,
    ConstraintKind,
    ConstraintFamilyView,
    ConstraintView,
    Instance,
)
from test_live_constraint_collections import make_model


def generic_family(model: Instance, kind: ConstraintKind[ConstraintIDBase]) -> None:
    assert_type(
        model.constraints.of_kind(kind),
        ConstraintFamilyView[ConstraintIDBase, ConstraintView],
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


def test_strings_are_not_family_selectors():
    model = make_model(Instance)
    with pytest.raises(TypeError, match="ConstraintKind"):
        model.constraints.of_kind("sos1")  # type: ignore[call-overload]
    with pytest.raises(TypeError, match="ConstraintKind"):
        model.constraints_df(kind="sos1")  # type: ignore[arg-type]
