"""Constraint mappings retain owners and resolve typed identities on demand."""

from collections.abc import Mapping
from copy import copy, deepcopy
from typing import Any, overload

import pytest
from typing_extensions import assert_type

from ommx import (
    ConstraintKind,
    AttachedConstraint,
    AttachedIndicatorConstraint,
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    ConstraintCollection,
    ConstraintID,
    ConstraintIDBase,
    ConstraintView,
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
    (ConstraintKind.Regular, ConstraintID, AttachedConstraint),
    (ConstraintKind.Indicator, IndicatorConstraintID, AttachedIndicatorConstraint),
    (ConstraintKind.OneHot, OneHotConstraintID, AttachedOneHotConstraint),
    (ConstraintKind.Sos1, Sos1ConstraintID, AttachedSos1Constraint),
]


NUMBERS = {id_type: 5 + i for i, (_, id_type, _) in enumerate(FAMILIES)}


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
        indicator_constraints={6: (y == 1).with_indicator(x)},
        one_hot_constraints={7: OneHotConstraint(variables=[x, y])},
        sos1_constraints={8: Sos1Constraint(variables=[x, y])},
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
    assert set(rows.keys()) == {id_type(NUMBERS[id_type]) for _, id_type, _ in FAMILIES}
    for kind, id_type, row_type in FAMILIES:
        row = rows[id_type(NUMBERS[id_type])]
        assert isinstance(row, row_type)
        assert row.constraint_id == id_type(NUMBERS[id_type])
        assert set(model.constraints_df(kind=kind).index) == {
            key for key in rows if isinstance(key, id_type)
        }
        row.set_name(str(kind))
        assert rows[id_type(NUMBERS[id_type])].name == str(kind)
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
        AttachedConstraint | None,
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
    generated = model.convert_one_hot_to_constraint(7)
    assert OneHotConstraintID(7) not in active
    assert OneHotConstraintID(7) in removed
    assert generated in active
    assert active[generated].provenance[0].original_id == OneHotConstraintID(7)
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
    model.relax_indicator_constraint(6, "manual")
    with pytest.raises(KeyError) as error:
        next(items)
    assert error.value.args == (IndicatorConstraintID(6),)


def test_all_collections_require_typed_keys_and_reject_structural_mutation():
    model = make_model(Instance)
    samples = model.evaluate_samples({9: {0: 1, 1: 0}})
    assert_type(samples.constraints, SampledConstraintCollection)
    for root in [model, make_model(ParametricInstance), samples, samples.get(9)]:
        assert not hasattr(root, "get_constraint_by_id")
        assert not hasattr(root, "get_removed_constraint_by_id")
        assert not hasattr(root, "get_constraints")
        assert not hasattr(root, "indicator_constraints")
        with pytest.raises(TypeError):
            root.constraints_df()  # type: ignore[call-arg]
        with pytest.raises(TypeError):
            root.constraints_df("regular")  # type: ignore[call-arg]
    assert not hasattr(samples, "summary_with_constraints")
    collections: list[Mapping[ConstraintIDBase, ConstraintView]] = [
        model.constraints,
        model.removed_constraints,
        make_model(ParametricInstance).constraints,
        make_model(ParametricInstance).removed_constraints,
        samples.constraints,
        samples.get(9).constraints,
    ]
    for rows in collections:
        assert rows == rows
        for key, row in rows.items():
            assert_type(key, ConstraintIDBase)
            assert_type(row, ConstraintView)
            assert isinstance(key, ConstraintIDBase)
            assert isinstance(row, ConstraintView)
            assert row.name is None
            assert row.description is None
            assert row.subscripts == []
            assert row.parameters == {}
            assert row.provenance == []
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


class FutureConstraintID(ConstraintIDBase):
    """Simulate a family introduced after this consumer was written."""


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_attached_equality_uses_owner_and_typed_id(host):
    model, other = make_model(host), make_model(host)
    rows = model.constraints
    assert rows == model.constraints
    assert rows == dict(rows)
    assert dict(rows) == rows
    assert rows != other.constraints
    for _, id_type, _ in FAMILIES:
        key = id_type(NUMBERS[id_type])
        row = rows[key]
        assert row == rows[key]
        assert row != other.constraints[key]
        assert row != object()
        assert row in rows.values()
        assert (key, row) in rows.items()
        row.set_name("changed through another handle")
        assert row == model.constraints[key]
    assert rows[ConstraintID(5)] != rows[OneHotConstraintID(7)]
    added = model.add_constraint(DecisionVariable.binary(0) <= 1)
    assert rows[ConstraintID(5)] != added


def test_result_equality_compares_snapshot_data_and_context():
    model = make_model(Instance)
    other = make_model(Instance)
    state = {0: 1, 1: 0}
    solution = model.evaluate(state)
    sampled = model.evaluate_samples({9: state})
    for left, right, changed in [
        (
            solution.constraints,
            other.evaluate(state).constraints,
            model.evaluate({0: 1, 1: 1}).constraints,
        ),
        (
            sampled.constraints,
            other.evaluate_samples({9: state}).constraints,
            model.evaluate_samples({9: {0: 1, 1: 1}}).constraints,
        ),
    ]:
        assert left == right
        assert left == dict(left)
        for key in left:
            assert left[key] == right[key]
            assert left[key] != changed[key]
            assert left[key] in left.values()
            assert (key, left[key]) in left.items()
    before = solution.constraints[ConstraintID(5)]
    solution.set_dual_variable(5, 2)
    assert before != solution.constraints[ConstraintID(5)]
    for row in other.constraints.values():
        assert isinstance(
            row,
            (
                AttachedConstraint,
                AttachedIndicatorConstraint,
                AttachedOneHotConstraint,
                AttachedSos1Constraint,
            ),
        )
        row.set_name("different context")
    for key in model.constraints:
        assert (
            model.evaluate(state).constraints[key]
            != other.evaluate(state).constraints[key]
        )
        assert (
            sampled.constraints[key]
            != other.evaluate_samples({9: state}).constraints[key]
        )


def remove_all(model: Instance) -> RemovedConstraintCollection:
    model.relax_constraint(5, "manual")
    model.relax_indicator_constraint(6, "manual")
    model.convert_one_hot_to_constraint(7)
    model.convert_sos1_to_constraints(8)
    return model.removed_constraints


def test_removed_equality_compares_snapshot_context_and_reason():
    model, other = make_model(Instance), make_model(Instance)
    rows, equivalent = remove_all(model), remove_all(other)
    assert rows == equivalent
    assert rows == dict(rows)
    for key in rows:
        assert rows[key] == equivalent[key]
        assert rows[key] in rows.values()
        assert (key, rows[key]) in rows.items()
    old = rows[ConstraintID(5)]
    model.restore_constraint(5)
    model.relax_constraint(5, "different reason")
    assert old != rows[ConstraintID(5)]
    # Result snapshots must also include retained lifecycle metadata.
    state = {0: 1, 1: 0}
    assert (
        model.evaluate(state).constraints[ConstraintID(5)]
        != other.evaluate(state).constraints[ConstraintID(5)]
    )
    assert (
        model.evaluate_samples({9: state}).constraints[ConstraintID(5)]
        != other.evaluate_samples({9: state}).constraints[ConstraintID(5)]
    )


@pytest.mark.parametrize("id_type", [family[1] for family in FAMILIES])
def test_typed_ids_copy_without_losing_the_family(id_type):
    key = id_type(5)
    assert copy(key) is key
    assert deepcopy(key) is key
    nested = {key: [key]}
    cloned = deepcopy(nested)
    assert cloned == nested
    assert cloned is not nested
    assert next(iter(cloned)) is key
    assert cloned[key][0] is key
    assert key != 5
    with pytest.raises(TypeError):
        int(key)


def test_mixed_lookup_errors_explain_typed_ids():
    model = make_model(Instance)
    for rows in [
        model.constraints,
        model.removed_constraints,
        model.evaluate({0: 1, 1: 0}).constraints,
        model.evaluate_samples({9: {0: 1, 1: 0}}).constraints,
    ]:
        for lookup in [
            lambda key: rows[key],
            lambda key: rows.get(key),
            lambda key: key in rows,
        ]:
            with pytest.raises(
                TypeError,
                match=r"Expected a typed constraint ID, got int\. Use ConstraintID\(n\)",
            ):
                lookup(5)
            with pytest.raises(
                TypeError, match="Unsupported constraint ID type: FutureConstraintID"
            ):
                lookup(FutureConstraintID())


def accept_current_families(key: ConstraintIDBase) -> None:
    if isinstance(
        key, (ConstraintID, IndicatorConstraintID, OneHotConstraintID, Sos1ConstraintID)
    ):
        return
    # Even after checking every currently known family, this is not Never.
    assert_type(key, ConstraintIDBase)
    raise NotImplementedError(f"Unsupported constraint family: {type(key).__name__}")


def test_new_family_requires_explicit_consumer_support():
    future = FutureConstraintID()
    with pytest.raises(NotImplementedError, match="FutureConstraintID"):
        accept_current_families(future)
    for root in [make_model(Instance), make_model(Instance).evaluate({0: 1, 1: 0})]:
        rows = root.constraints
        # An unrecognized family must not look like a missing ID in a known family.
        with pytest.raises(TypeError):
            rows[future]
        with pytest.raises(TypeError):
            rows.get(future, "missing")


def test_removed_rows_share_the_open_metadata_interface():
    model = make_model(Instance)
    indicator = model.constraints[IndicatorConstraintID(6)]
    indicator.set_name("retained indicator")
    indicator.set_description("metadata survives removal")
    indicator.set_subscripts([2])
    indicator.set_parameters({"group": "test"})
    model.relax_constraint(5, "manual")
    model.relax_indicator_constraint(6, "manual")
    model.convert_one_hot_to_constraint(7)
    model.convert_sos1_to_constraints(8)
    assert len(model.removed_constraints) == 4
    for key, row in model.removed_constraints.items():
        assert_type(key, ConstraintIDBase)
        assert_type(row, ConstraintView)
        assert isinstance(row, ConstraintView)
        if isinstance(key, IndicatorConstraintID):
            assert row.name == "retained indicator"
            assert row.description == "metadata survives removal"
            assert row.subscripts == [2]
            assert row.parameters == {"group": "test"}
        else:
            assert row.name is None
            assert row.description is None
            assert row.subscripts == []
            assert row.parameters == {}
        assert row.provenance == []
