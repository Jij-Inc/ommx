"""Concrete get overloads preserve row and default types on every collection."""

from collections.abc import Mapping
from typing_extensions import assert_type
from ommx import (
    ConstraintIDBase,
    ConstraintView,
    Instance,
    ConstraintID,
    IndicatorConstraintID,
    OneHotConstraintID,
    Sos1ConstraintID,
    AttachedConstraint,
    AttachedIndicatorConstraint,
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    InactiveConstraint,
    InactiveIndicatorConstraint,
    InactiveOneHotConstraint,
    InactiveSos1Constraint,
    EvaluatedConstraint,
    EvaluatedIndicatorConstraint,
    EvaluatedOneHotConstraint,
    EvaluatedSos1Constraint,
    SampledConstraint,
    SampledIndicatorConstraint,
    SampledOneHotConstraint,
    SampledSos1Constraint,
)
from test_live_constraint_collections import make_model, remove_all


class Missing:
    pass


def check_open_key(
    model: Instance, key: ConstraintIDBase, default: ConstraintView
) -> None:
    assert_type(model.active_constraints.get(key), ConstraintView | None)
    assert_type(model.active_constraints.get(key, Missing()), ConstraintView | Missing)
    assert_type(model.active_constraints.get(key, default), ConstraintView)
    # A concrete row unioned with its base remains assignable to that base.
    value: ConstraintView = model.active_constraints.get(ConstraintID(5), default)
    assert isinstance(value, ConstraintView)
    mapping: Mapping[ConstraintIDBase, ConstraintView] = model.active_constraints
    assert_type(mapping.get(key, Missing()), ConstraintView | Missing)
    assert mapping.get(key, default) == model.active_constraints.get(key, default)


def test_get_types():
    model = make_model(Instance)
    active = model.active_constraints
    evaluated = model.evaluate({0: 1, 1: 0}).constraints
    sampled = model.evaluate_samples({9: {0: 1, 1: 0}}).constraints
    removed = remove_all(make_model(Instance))
    check_open_key(model, ConstraintID(5), active[ConstraintID(5)])
    assert_type(active.get(ConstraintID(5)), AttachedConstraint | None)
    assert_type(active.get(ConstraintID(5), None), AttachedConstraint | None)
    assert_type(active.get(ConstraintID(999), Missing()), AttachedConstraint | Missing)
    assert_type(
        active.get(IndicatorConstraintID(6)), AttachedIndicatorConstraint | None
    )
    assert_type(
        active.get(IndicatorConstraintID(6), None), AttachedIndicatorConstraint | None
    )
    assert_type(
        active.get(IndicatorConstraintID(999), Missing()),
        AttachedIndicatorConstraint | Missing,
    )
    assert_type(active.get(OneHotConstraintID(7)), AttachedOneHotConstraint | None)
    assert_type(
        active.get(OneHotConstraintID(7), None), AttachedOneHotConstraint | None
    )
    assert_type(
        active.get(OneHotConstraintID(999), Missing()),
        AttachedOneHotConstraint | Missing,
    )
    assert_type(active.get(Sos1ConstraintID(8)), AttachedSos1Constraint | None)
    assert_type(active.get(Sos1ConstraintID(8), None), AttachedSos1Constraint | None)
    assert_type(
        active.get(Sos1ConstraintID(999), Missing()), AttachedSos1Constraint | Missing
    )
    assert_type(removed.get(ConstraintID(5)), InactiveConstraint | None)
    assert_type(removed.get(ConstraintID(5), None), InactiveConstraint | None)
    assert_type(removed.get(ConstraintID(999), Missing()), InactiveConstraint | Missing)
    assert_type(
        removed.get(IndicatorConstraintID(6)), InactiveIndicatorConstraint | None
    )
    assert_type(
        removed.get(IndicatorConstraintID(6), None), InactiveIndicatorConstraint | None
    )
    assert_type(
        removed.get(IndicatorConstraintID(999), Missing()),
        InactiveIndicatorConstraint | Missing,
    )
    assert_type(removed.get(OneHotConstraintID(7)), InactiveOneHotConstraint | None)
    assert_type(
        removed.get(OneHotConstraintID(7), None), InactiveOneHotConstraint | None
    )
    assert_type(
        removed.get(OneHotConstraintID(999), Missing()),
        InactiveOneHotConstraint | Missing,
    )
    assert_type(removed.get(Sos1ConstraintID(8)), InactiveSos1Constraint | None)
    assert_type(removed.get(Sos1ConstraintID(8), None), InactiveSos1Constraint | None)
    assert_type(
        removed.get(Sos1ConstraintID(999), Missing()), InactiveSos1Constraint | Missing
    )
    assert_type(evaluated.get(ConstraintID(5)), EvaluatedConstraint | None)
    assert_type(evaluated.get(ConstraintID(5), None), EvaluatedConstraint | None)
    assert_type(
        evaluated.get(ConstraintID(999), Missing()), EvaluatedConstraint | Missing
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(6)), EvaluatedIndicatorConstraint | None
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(6), None),
        EvaluatedIndicatorConstraint | None,
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(999), Missing()),
        EvaluatedIndicatorConstraint | Missing,
    )
    assert_type(evaluated.get(OneHotConstraintID(7)), EvaluatedOneHotConstraint | None)
    assert_type(
        evaluated.get(OneHotConstraintID(7), None), EvaluatedOneHotConstraint | None
    )
    assert_type(
        evaluated.get(OneHotConstraintID(999), Missing()),
        EvaluatedOneHotConstraint | Missing,
    )
    assert_type(evaluated.get(Sos1ConstraintID(8)), EvaluatedSos1Constraint | None)
    assert_type(
        evaluated.get(Sos1ConstraintID(8), None), EvaluatedSos1Constraint | None
    )
    assert_type(
        evaluated.get(Sos1ConstraintID(999), Missing()),
        EvaluatedSos1Constraint | Missing,
    )
    assert_type(sampled.get(ConstraintID(5)), SampledConstraint | None)
    assert_type(sampled.get(ConstraintID(5), None), SampledConstraint | None)
    assert_type(sampled.get(ConstraintID(999), Missing()), SampledConstraint | Missing)
    assert_type(
        sampled.get(IndicatorConstraintID(6)), SampledIndicatorConstraint | None
    )
    assert_type(
        sampled.get(IndicatorConstraintID(6), None), SampledIndicatorConstraint | None
    )
    assert_type(
        sampled.get(IndicatorConstraintID(999), Missing()),
        SampledIndicatorConstraint | Missing,
    )
    assert_type(sampled.get(OneHotConstraintID(7)), SampledOneHotConstraint | None)
    assert_type(
        sampled.get(OneHotConstraintID(7), None), SampledOneHotConstraint | None
    )
    assert_type(
        sampled.get(OneHotConstraintID(999), Missing()),
        SampledOneHotConstraint | Missing,
    )
    assert_type(sampled.get(Sos1ConstraintID(8)), SampledSos1Constraint | None)
    assert_type(sampled.get(Sos1ConstraintID(8), None), SampledSos1Constraint | None)
    assert_type(
        sampled.get(Sos1ConstraintID(999), Missing()), SampledSos1Constraint | Missing
    )
