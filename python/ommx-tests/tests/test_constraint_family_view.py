"""Family views preserve typed identities and the owner's lifecycle semantics."""

from collections.abc import Mapping
from typing import Any, get_origin

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    AttachedIndicatorConstraint,
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    ConstraintFamilyView,
    ConstraintID,
    DecisionVariable,
    EvaluatedConstraint,
    EvaluatedIndicatorConstraint,
    EvaluatedOneHotConstraint,
    EvaluatedSos1Constraint,
    Instance,
    ParametricInstance,
    RemovedConstraint,
    RemovedIndicatorConstraint,
    RemovedOneHotConstraint,
    RemovedSos1Constraint,
    SampledConstraint,
    SampledIndicatorConstraint,
    SampledOneHotConstraint,
    SampledSos1Constraint,
    Sos1ConstraintID,
)
from test_live_constraint_collections import (
    FAMILIES,
    FutureConstraintID,
    make_model,
    remove_all,
)


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
@pytest.mark.parametrize("kind,id_type,row_type", FAMILIES)
def test_family_views_filter_and_promote_integers(host, kind, id_type, row_type):
    model = make_model(host)
    rows = model.constraints.of_kind(kind)
    assert isinstance(rows, ConstraintFamilyView)
    assert isinstance(rows, Mapping)
    assert len(rows) == 1
    assert list(rows) == [id_type(5)]
    assert isinstance(rows[5], row_type)
    assert rows[5] == rows[id_type(5)] == rows.get(5)
    assert 5 in rows and id_type(5) in rows
    assert set(rows.keys()) == set(model.constraints_df(kind=kind).index)
    assert list(rows.values()) == [rows[5]]
    assert list(rows.items()) == [(id_type(5), rows[5])]
    assert rows == dict(rows) == rows
    assert rows != {5: rows[5]}
    assert rows == rows
    rows[5].set_name("changed through family view")
    assert model.constraints[id_type(5)].name == "changed through family view"
    del model
    assert rows[5].name == "changed through family view"


@pytest.mark.parametrize("kind,id_type,_", FAMILIES)
def test_every_lifecycle_uses_same_key_rules(kind, id_type, _):
    model = make_model(Instance)
    solution = model.evaluate({0: 1, 1: 0})
    samples = model.evaluate_samples({9: {0: 1, 1: 0}})
    removed = remove_all(model)
    collections = [
        make_model(Instance).constraints,
        make_model(ParametricInstance).constraints,
        removed,
        model.as_parametric_instance().removed_constraints,
        solution.constraints,
        samples.constraints,
    ]
    for parent in collections:
        rows = parent.of_kind(kind)
        assert len(rows) == 1
        assert rows[5] == parent[id_type(5)]
        assert list(rows) == [id_type(5)]
        assert rows.get(999) is None
        sentinel = object()
        assert rows.get(999, sentinel) is sentinel
        assert 999 not in rows
        with pytest.raises(KeyError) as error:
            rows[999]
        assert error.value.args == (id_type(999),)
        wrong_keys: list[Any] = [True, "5", 5.0, FutureConstraintID()]
        wrong_keys += [other(5) for _, other, _ in FAMILIES if other is not id_type]
        for key in wrong_keys:
            with pytest.raises(TypeError):
                rows[key]
            with pytest.raises(TypeError):
                rows.get(key, sentinel)
            with pytest.raises(TypeError):
                _ = key in rows
        for key in [-1, 2**64]:
            with pytest.raises(OverflowError):
                rows[key]
        with pytest.raises(TypeError):
            parent[5]  # type: ignore[call-overload]
        with pytest.raises(TypeError):
            rows[5] = rows[5]  # type: ignore[index]
        with pytest.raises(TypeError):
            del rows[5]  # type: ignore[index]
        with pytest.raises(ValueError, match="unknown constraint kind"):
            parent.of_kind("unknown")  # type: ignore[call-overload]
        with pytest.raises(TypeError):
            parent.of_kind(1)  # type: ignore[call-overload]


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_live_views_and_iteration_timing(host):
    model = make_model(host)
    rows = model.constraints.of_kind("regular")
    keys, values, items = rows.keys(), rows.values(), rows.items()
    direct = iter(rows)
    key_iterator = iter(keys)
    added = model.add_constraint(DecisionVariable.binary(0) <= 1)
    added.set_name("later")
    assert list(direct) == [ConstraintID(5)]
    assert added.constraint_id in list(key_iterator)
    assert len(rows) == len(keys) == len(values) == len(items) == 2
    assert any(row.name == "later" for row in values)
    assert len(model.constraints.of_kind("sos1")) == 1


def test_removal_and_restoration_update_family_views():
    model = make_model(Instance)
    active = model.constraints.of_kind("regular")
    removed = model.removed_constraints.of_kind("regular")
    assert not removed
    model.relax_constraint(5, "manual")
    assert not active
    snapshot = removed[5]
    assert snapshot.removed_reason == "manual"
    model.restore_constraint(5)
    assert not removed
    assert 5 in active
    assert snapshot.removed_reason == "manual"
    sos1 = model.constraints.of_kind("sos1")
    removed_sos1 = model.removed_constraints.of_kind("sos1")
    model.convert_sos1_to_constraints(5)
    assert not sos1
    assert 5 in removed_sos1
    # Result views retain removed rows just like their parent collections.
    assert 5 in model.evaluate({0: 1, 1: 0}).constraints.of_kind("sos1")


def test_result_rows_are_fetched_lazily_and_owner_is_retained():
    solution = make_model(Instance).evaluate({0: 1, 1: 0})
    rows = solution.constraints.of_kind("regular")
    values = rows.values()
    snapshot = rows[5]
    solution.set_dual_variable(5, 3)
    assert snapshot.dual_variable is None
    assert next(iter(values)).dual_variable == 3
    del solution
    assert rows[5].dual_variable == 3


def test_precise_types_and_runtime_generic_alias():
    model = make_model(Instance)
    rows = model.constraints.of_kind("sos1")
    assert_type(rows, ConstraintFamilyView[Sos1ConstraintID, AttachedSos1Constraint])
    assert_type(rows[5], AttachedSos1Constraint)
    assert_type(rows[Sos1ConstraintID(5)], AttachedSos1Constraint)
    assert_type(next(iter(rows)), Sos1ConstraintID)
    assert_type(next(iter(rows.keys())), Sos1ConstraintID)
    assert_type(next(iter(rows.values())), AttachedSos1Constraint)
    assert_type(
        next(iter(rows.items())), tuple[Sos1ConstraintID, AttachedSos1Constraint]
    )
    assert_type(rows.get(5), AttachedSos1Constraint | None)
    assert_type(rows.get(999, rows[5]), AttachedSos1Constraint)
    assert_type(rows.get(999, "missing"), AttachedSos1Constraint | str)
    mapping: Mapping[Sos1ConstraintID, AttachedSos1Constraint] = rows
    assert mapping[Sos1ConstraintID(5)] == rows[5]
    alias = ConstraintFamilyView[Sos1ConstraintID, AttachedSos1Constraint]
    assert get_origin(alias) is ConstraintFamilyView
    assert_type(model.constraints.of_kind("regular")[5], AttachedConstraint)
    assert_type(model.constraints.of_kind("indicator")[5], AttachedIndicatorConstraint)
    assert_type(model.constraints.of_kind("one_hot")[5], AttachedOneHotConstraint)
    solution = model.evaluate({0: 1, 1: 0})
    assert_type(solution.constraints.of_kind("regular")[5], EvaluatedConstraint)
    assert_type(
        solution.constraints.of_kind("indicator")[5], EvaluatedIndicatorConstraint
    )
    assert_type(solution.constraints.of_kind("one_hot")[5], EvaluatedOneHotConstraint)
    assert_type(solution.constraints.of_kind("sos1")[5], EvaluatedSos1Constraint)
    samples = model.evaluate_samples({9: {0: 1, 1: 0}})
    assert_type(samples.constraints.of_kind("regular")[5], SampledConstraint)
    assert_type(samples.constraints.of_kind("indicator")[5], SampledIndicatorConstraint)
    assert_type(samples.constraints.of_kind("one_hot")[5], SampledOneHotConstraint)
    assert_type(samples.constraints.of_kind("sos1")[5], SampledSos1Constraint)
    removed = remove_all(model)
    assert_type(removed.of_kind("regular")[5], RemovedConstraint)
    assert_type(removed.of_kind("indicator")[5], RemovedIndicatorConstraint)
    assert_type(removed.of_kind("one_hot")[5], RemovedOneHotConstraint)
    assert_type(removed.of_kind("sos1")[5], RemovedSos1Constraint)
