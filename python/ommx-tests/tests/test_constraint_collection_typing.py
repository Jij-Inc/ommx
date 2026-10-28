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
    RemovedConstraint,
    RemovedIndicatorConstraint,
    RemovedOneHotConstraint,
    RemovedSos1Constraint,
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
    assert_type(model.constraints.get(key), ConstraintView | None)
    assert_type(model.constraints.get(key, Missing()), ConstraintView | Missing)
    assert_type(model.constraints.get(key, default), ConstraintView)
    # A concrete row unioned with its base remains assignable to that base.
    value: ConstraintView = model.constraints.get(ConstraintID(5), default)
    assert isinstance(value, ConstraintView)
    mapping: Mapping[ConstraintIDBase, ConstraintView] = model.constraints
    assert_type(mapping.get(key, Missing()), ConstraintView | Missing)
    assert mapping.get(key, default) == model.constraints.get(key, default)


def test_get_types():
    model = make_model(Instance)
    active = model.constraints
    evaluated = model.evaluate({0: 1, 1: 0}).constraints
    sampled = model.evaluate_samples({9: {0: 1, 1: 0}}).constraints
    removed = remove_all(make_model(Instance))
    check_open_key(model, ConstraintID(5), active[ConstraintID(5)])
    assert_type(active.get(ConstraintID(5)), AttachedConstraint | None)
    assert_type(active.get(ConstraintID(5), None), AttachedConstraint | None)
    assert_type(active.get(ConstraintID(999), Missing()), AttachedConstraint | Missing)
    assert_type(
        active.get(IndicatorConstraintID(5)), AttachedIndicatorConstraint | None
    )
    assert_type(
        active.get(IndicatorConstraintID(5), None), AttachedIndicatorConstraint | None
    )
    assert_type(
        active.get(IndicatorConstraintID(999), Missing()),
        AttachedIndicatorConstraint | Missing,
    )
    assert_type(active.get(OneHotConstraintID(5)), AttachedOneHotConstraint | None)
    assert_type(
        active.get(OneHotConstraintID(5), None), AttachedOneHotConstraint | None
    )
    assert_type(
        active.get(OneHotConstraintID(999), Missing()),
        AttachedOneHotConstraint | Missing,
    )
    assert_type(active.get(Sos1ConstraintID(5)), AttachedSos1Constraint | None)
    assert_type(active.get(Sos1ConstraintID(5), None), AttachedSos1Constraint | None)
    assert_type(
        active.get(Sos1ConstraintID(999), Missing()), AttachedSos1Constraint | Missing
    )
    assert_type(removed.get(ConstraintID(5)), RemovedConstraint | None)
    assert_type(removed.get(ConstraintID(5), None), RemovedConstraint | None)
    assert_type(removed.get(ConstraintID(999), Missing()), RemovedConstraint | Missing)
    assert_type(
        removed.get(IndicatorConstraintID(5)), RemovedIndicatorConstraint | None
    )
    assert_type(
        removed.get(IndicatorConstraintID(5), None), RemovedIndicatorConstraint | None
    )
    assert_type(
        removed.get(IndicatorConstraintID(999), Missing()),
        RemovedIndicatorConstraint | Missing,
    )
    assert_type(removed.get(OneHotConstraintID(5)), RemovedOneHotConstraint | None)
    assert_type(
        removed.get(OneHotConstraintID(5), None), RemovedOneHotConstraint | None
    )
    assert_type(
        removed.get(OneHotConstraintID(999), Missing()),
        RemovedOneHotConstraint | Missing,
    )
    assert_type(removed.get(Sos1ConstraintID(5)), RemovedSos1Constraint | None)
    assert_type(removed.get(Sos1ConstraintID(5), None), RemovedSos1Constraint | None)
    assert_type(
        removed.get(Sos1ConstraintID(999), Missing()), RemovedSos1Constraint | Missing
    )
    assert_type(evaluated.get(ConstraintID(5)), EvaluatedConstraint | None)
    assert_type(evaluated.get(ConstraintID(5), None), EvaluatedConstraint | None)
    assert_type(
        evaluated.get(ConstraintID(999), Missing()), EvaluatedConstraint | Missing
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(5)), EvaluatedIndicatorConstraint | None
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(5), None),
        EvaluatedIndicatorConstraint | None,
    )
    assert_type(
        evaluated.get(IndicatorConstraintID(999), Missing()),
        EvaluatedIndicatorConstraint | Missing,
    )
    assert_type(evaluated.get(OneHotConstraintID(5)), EvaluatedOneHotConstraint | None)
    assert_type(
        evaluated.get(OneHotConstraintID(5), None), EvaluatedOneHotConstraint | None
    )
    assert_type(
        evaluated.get(OneHotConstraintID(999), Missing()),
        EvaluatedOneHotConstraint | Missing,
    )
    assert_type(evaluated.get(Sos1ConstraintID(5)), EvaluatedSos1Constraint | None)
    assert_type(
        evaluated.get(Sos1ConstraintID(5), None), EvaluatedSos1Constraint | None
    )
    assert_type(
        evaluated.get(Sos1ConstraintID(999), Missing()),
        EvaluatedSos1Constraint | Missing,
    )
    assert_type(sampled.get(ConstraintID(5)), SampledConstraint | None)
    assert_type(sampled.get(ConstraintID(5), None), SampledConstraint | None)
    assert_type(sampled.get(ConstraintID(999), Missing()), SampledConstraint | Missing)
    assert_type(
        sampled.get(IndicatorConstraintID(5)), SampledIndicatorConstraint | None
    )
    assert_type(
        sampled.get(IndicatorConstraintID(5), None), SampledIndicatorConstraint | None
    )
    assert_type(
        sampled.get(IndicatorConstraintID(999), Missing()),
        SampledIndicatorConstraint | Missing,
    )
    assert_type(sampled.get(OneHotConstraintID(5)), SampledOneHotConstraint | None)
    assert_type(
        sampled.get(OneHotConstraintID(5), None), SampledOneHotConstraint | None
    )
    assert_type(
        sampled.get(OneHotConstraintID(999), Missing()),
        SampledOneHotConstraint | Missing,
    )
    assert_type(sampled.get(Sos1ConstraintID(5)), SampledSos1Constraint | None)
    assert_type(sampled.get(Sos1ConstraintID(5), None), SampledSos1Constraint | None)
    assert_type(
        sampled.get(Sos1ConstraintID(999), Missing()), SampledSos1Constraint | Missing
    )
