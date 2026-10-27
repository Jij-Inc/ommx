"""Constraint mappings retain owners and resolve typed identities on demand."""

from collections.abc import Mapping
from typing import Any, overload

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    AttachedIndicatorConstraint,
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    ConstraintCollection,
    ConstraintID,
    DecisionVariable,
    EvaluatedConstraintCollection,
    EvaluatedConstraint,
    IndicatorConstraintID,
    Instance,
    OneHotConstraint,
    OneHotConstraintID,
    ParametricInstance,
    RemovedConstraint,
    RemovedConstraintCollection,
    SampledConstraintCollection,
    Sense,
    Sos1Constraint,
    Sos1ConstraintID,
)

FAMILIES = [
    ("regular", ConstraintID, AttachedConstraint),
    ("indicator", IndicatorConstraintID, AttachedIndicatorConstraint),
    ("one_hot", OneHotConstraintID, AttachedOneHotConstraint),
    ("sos1", Sos1ConstraintID, AttachedSos1Constraint),
]


@overload
def make_model(host: type[Instance]) -> Instance: ...


@overload
def make_model(host: type[ParametricInstance]) -> ParametricInstance: ...


def make_model(host: type[Instance] | type[ParametricInstance]):
    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    kwargs: dict[str, Any] = dict(
        sense=Sense.Minimize,
        objective=x + y,
        decision_variables=[x, y],
        constraints={5: x + y == 1},
        indicator_constraints={5: (y == 1).with_indicator(x)},
        one_hot_constraints={5: OneHotConstraint(variables=[x, y])},
        sos1_constraints={5: Sos1Constraint(variables=[x, y])},
    )
    if host is ParametricInstance:
        kwargs["parameters"] = []
    return host.from_components(**kwargs)


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_all_families_share_one_live_mapping(host):
    model = make_model(host)
    rows = model.constraints
    assert isinstance(rows, Mapping)
    assert isinstance(rows, ConstraintCollection)
    assert rows == rows
    assert len(rows) == 4
    assert set(rows.keys()) == {id_type(5) for _, id_type, _ in FAMILIES}
    for kind, id_type, row_type in FAMILIES:
        row = rows[id_type(5)]
        assert isinstance(row, row_type)
        assert row.constraint_id == id_type(5)
        assert set(model.constraints_df(kind=kind).index) == {
            key for key in rows if isinstance(key, id_type)
        }
        row.set_name(kind)
        assert rows[id_type(5)].name == kind
    assert len(list(rows.values())) == len(list(rows.items())) == 4
    assert set(dict(rows)) == set(rows)


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_retained_views_observe_additions_and_keep_owner_alive(host):
    model = make_model(host)
    rows = model.constraints
    keys, values, items = rows.keys(), rows.values(), rows.items()
    iterator = iter(rows)
    added = model.add_constraint(DecisionVariable.binary(0) <= 1)
    added.set_name("added later")
    assert added.constraint_id in rows
    assert added.constraint_id in keys
    assert len(values) == len(items) == 5
    assert any(row.name == "added later" for row in values)
    assert added.constraint_id not in list(
        iterator
    )  # Keys snapshot at iteration start.
    del added, model
    assert rows[ConstraintID(5)].constraint_id == ConstraintID(5)
    assert len(rows) == 5


def test_removal_restoration_and_lowering_update_retained_views():
    model = make_model(Instance)
    active, removed = model.constraints, model.removed_constraints
    assert_type(active, ConstraintCollection)
    assert_type(removed, RemovedConstraintCollection)
    assert_type(
        active.get(ConstraintID(5)),
        AttachedConstraint
        | AttachedIndicatorConstraint
        | AttachedOneHotConstraint
        | AttachedSos1Constraint
        | None,
    )
    model.relax_constraint(5, "manual")
    assert ConstraintID(5) not in active
    assert ConstraintID(5) in removed
    assert_type(removed[ConstraintID(5)], RemovedConstraint)
    assert removed[ConstraintID(5)].removed_reason == "manual"
    with pytest.raises(KeyError) as error:
        active[ConstraintID(5)]
    assert error.value.args == (ConstraintID(5),)
    model.restore_constraint(5)
    assert ConstraintID(5) in active
    assert ConstraintID(5) not in removed
    generated = model.convert_one_hot_to_constraint(5)
    assert OneHotConstraintID(5) not in active
    assert OneHotConstraintID(5) in removed
    assert generated in active
    assert active[generated].provenance[0].original_id == OneHotConstraintID(5)
    for kind, id_type, _ in FAMILIES:
        expected = {key for key in active if isinstance(key, id_type)} | {
            key for key in removed if isinstance(key, id_type)
        }
        assert set(model.constraints_df(kind=kind, removed=True).index) == expected


def test_result_rows_are_fetched_at_access_time():
    model = make_model(Instance)
    solution = model.evaluate({0: 1, 1: 0})
    rows = solution.constraints
    assert_type(rows, EvaluatedConstraintCollection)
    values = rows.values()
    snapshot = rows[ConstraintID(5)]
    solution.set_dual_variable(5, 3)
    assert snapshot.dual_variable is None
    assert rows[ConstraintID(5)].dual_variable == 3
    row = next(iter(values))
    assert isinstance(row, EvaluatedConstraint)
    assert row.dual_variable == 3
    snapshot.dual_variable = 8
    assert rows[ConstraintID(5)].dual_variable == 3
    del solution, model
    assert rows[ConstraintID(5)].dual_variable == 3


def test_value_iteration_reports_ids_removed_after_iteration_started():
    model = make_model(Instance)
    items = iter(model.constraints.items())
    assert next(items)[0] == ConstraintID(5)
    model.relax_indicator_constraint(5, "manual")
    with pytest.raises(KeyError) as error:
        next(items)
    assert error.value.args == (IndicatorConstraintID(5),)


def test_all_collections_require_typed_keys_and_reject_structural_mutation():
    model = make_model(Instance)
    samples = model.evaluate_samples({9: {0: 1, 1: 0}})
    assert_type(samples.constraints, SampledConstraintCollection)
    for root in [model, make_model(ParametricInstance), samples, samples.get(9)]:
        assert not hasattr(root, "get_constraints")
        assert not hasattr(root, "indicator_constraints")
        with pytest.raises(TypeError):
            root.constraints_df()  # type: ignore[call-arg]
        with pytest.raises(TypeError):
            root.constraints_df("regular")  # type: ignore[call-arg]
    for rows in [
        model.constraints,
        model.removed_constraints,
        make_model(ParametricInstance).constraints,
        make_model(ParametricInstance).removed_constraints,
        samples.constraints,
        samples.get(9).constraints,
    ]:
        assert rows == rows
        for key in [5, True, "5"]:
            with pytest.raises(TypeError):
                rows[key]  # type: ignore[index]
            with pytest.raises(TypeError):
                _ = key in rows
            with pytest.raises(TypeError):
                rows.get(key)  # type: ignore[call-overload]
        for _, id_type, _ in FAMILIES:
            assert id_type(999) not in rows
            assert rows.get(id_type(999)) is None
            sentinel = object()
            assert rows.get(id_type(999), sentinel) is sentinel
            with pytest.raises(KeyError):
                rows[id_type(999)]
        with pytest.raises(TypeError):
            rows[ConstraintID(5)] = None  # type: ignore[index]
        with pytest.raises(TypeError):
            del rows[ConstraintID(5)]  # type: ignore[index]
