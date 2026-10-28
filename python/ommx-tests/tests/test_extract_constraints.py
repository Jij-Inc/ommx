"""Name extraction shares label semantics across stages and constraint families."""

from itertools import combinations
from typing import TypeVar

import pytest
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    ConstraintID,
    ConstraintView,
    DecisionVariable,
    IndicatorConstraintID,
    Instance,
    Parameter,
    ParametricInstance,
    SampleSet,
    Sense,
    Solution,
)
from test_live_constraint_collections import FAMILIES, make_model, remove_all

Model = TypeVar("Model", bound=Instance | ParametricInstance)


def label_model(model: Model) -> Model:
    for index, (_, id_type, _) in enumerate(FAMILIES):
        row = model.constraints[id_type(5)]
        row.set_name("c")
        row.set_subscripts([index, -1])
        row.set_parameters({"group": str(index)})
    return model


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_model_extraction_returns_attached_rows_of_all_families(
    host: type[Instance] | type[ParametricInstance],
):
    model = label_model(make_model(host))
    extracted = model.extract_constraints("c")
    assert_type(extracted, dict[tuple[int, ...], ConstraintView])
    assert set(extracted) == {(index, -1) for index in range(4)}
    row: ConstraintView | None = None
    for index, (_, id_type, row_type) in enumerate(FAMILIES):
        row = extracted[index, -1]
        assert type(row) is row_type
        assert row == model.constraints[id_type(5)]

    regular = extracted[0, -1]
    assert isinstance(regular, AttachedConstraint)
    regular.set_name("renamed")
    regular.set_subscripts([42])
    assert model.constraints[ConstraintID(5)].name == "renamed"
    assert model.extract_constraints("renamed") == {(42,): regular}
    assert (0, -1) in extracted  # Dictionary keys snapshot the original labels.
    assert set(model.extract_constraints("c")) == {(i, -1) for i in range(1, 4)}

    indicator = extracted[1, -1]
    del extracted, regular, row, model
    assert indicator.name == "c"  # Extracted handles retain their owner.


def test_model_extraction_excludes_removed_rows_on_both_roots():
    model = label_model(make_model(Instance))
    model.relax_constraint(5, "manual")
    for root in [model, model.as_parametric_instance()]:
        assert set(root.extract_constraints("c")) == {(i, -1) for i in range(1, 4)}
        assert ConstraintID(5) in root.removed_constraints


@pytest.mark.parametrize("removed", [False, True])
def test_results_extract_violations_including_retained_removed_constraints(removed):
    model = label_model(make_model(Instance))
    if removed:
        remove_all(model)
        # Lowering copies labels to generated rows. Retained source rows are
        # also searched, so this ambiguity must fail rather than overwrite.
        with pytest.raises(ValueError, match="Duplicate subscript"):
            model.evaluate({0: 0, 1: 0}).extract_constraints("c")
        with pytest.raises(ValueError, match="Duplicate subscript"):
            model.evaluate_samples({7: {0: 0, 1: 0}}).extract_constraints("c", 7)
        for row in model.constraints.values():
            assert isinstance(row, AttachedConstraint)
            row.set_name("lowered")
    samples = model.evaluate_samples({7: {0: 0, 1: 0}, 42: {0: 1, 1: 1}})
    # Regular f(x)=-1 is a violation of +1; an inactive indicator has zero
    # violation even though its body is -1. One-hot/SOS1 use their own metrics.
    expected = {
        7: {(0, -1): 1.0, (1, -1): 0.0, (2, -1): 1.0, (3, -1): 0.0},
        42: {(0, -1): 1.0, (1, -1): 0.0, (2, -1): 1.0, (3, -1): 1.0},
    }
    for sample_id, state in [(7, {0: 0, 1: 0}), (42, {0: 1, 1: 1})]:
        direct = model.evaluate(state)
        extracted = direct.extract_constraints("c")
        assert_type(extracted, dict[tuple[int, ...], float])
        assert extracted == expected[sample_id]
        values = samples.extract_constraints("c", sample_id)
        assert_type(values, dict[tuple[int, ...], float])
        assert values == extracted
        assert samples.get(sample_id).extract_constraints("c") == extracted
        assert (
            type(direct).from_v2_bytes(direct.to_v2_bytes()).extract_constraints("c")
            == extracted
        )
        assert (
            type(samples)
            .from_v2_bytes(samples.to_v2_bytes())
            .extract_constraints("c", sample_id)
            == extracted
        )


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
@pytest.mark.parametrize("left,right", list(combinations(range(4), 2)))
def test_duplicate_subscripts_across_every_family_pair_are_rejected(host, left, right):
    model = label_model(make_model(host))
    model.constraints[FAMILIES[right][1](5)].set_subscripts([left, -1])
    with pytest.raises(ValueError, match="Duplicate subscript"):
        model.extract_constraints("c")
    if isinstance(model, ParametricInstance):
        model = model.with_parameters({})
    with pytest.raises(ValueError, match="Duplicate subscript"):
        model.evaluate({0: 1, 1: 0}).extract_constraints("c")
    with pytest.raises(ValueError, match="Duplicate subscript"):
        model.evaluate_samples({7: {0: 1, 1: 0}}).extract_constraints("c", 7)


def test_scalar_labels_missing_names_and_invalid_sample_ids():
    model = make_model(Instance)
    for index, (kind, id_type, _) in enumerate(FAMILIES):
        model.constraints[id_type(5)].set_name(kind)
        assert set(model.extract_constraints(kind)) == {()}
    with pytest.raises(KeyError):
        model.extract_constraints("missing")
    with pytest.raises(KeyError):
        make_model(ParametricInstance).extract_constraints("missing")
    solution = model.evaluate({0: 0, 1: 0})
    assert solution.extract_constraints("regular") == {(): 1.0}
    assert solution.extract_constraints("indicator") == {(): 0.0}
    with pytest.raises(KeyError):
        solution.extract_constraints("missing")
    samples = model.evaluate_samples({7: {0: 0, 1: 0}})
    for name in ["missing", "regular", "indicator", "one_hot", "sos1"]:
        with pytest.raises(KeyError):
            samples.extract_constraints(name, 999)
    with pytest.raises(KeyError):
        samples.extract_constraints("missing", 7)


def test_feasible_regular_inequality_and_inactive_indicator_return_zero():
    model = Instance.minimize()
    x = model.new_binary("x")
    model.add_constraint(x - 3 <= 0, "slack")
    assert model.evaluate({x.id: 0}).extract_constraints("slack") == {(): 0.0}

    other = label_model(make_model(Instance))
    solution = other.evaluate({0: 0, 1: 0})
    assert solution.constraints[IndicatorConstraintID(5)].evaluated_value == -1
    assert solution.extract_constraints("c")[1, -1] == 0


def test_constraint_names_match_extraction_on_every_root():
    model = make_model(Instance)
    assert model.constraint_names == set()  # All four rows are unnamed.
    assert model.as_parametric_instance().constraint_names == set()
    assert model.evaluate({0: 0, 1: 0}).constraint_names == set()
    assert model.evaluate_samples({7: {0: 0, 1: 0}}).constraint_names == set()

    names = ["regular", "", "one_hot", "sos1"]
    for name, (_, id_type, _) in zip(names, FAMILIES):
        model.constraints[id_type(5)].set_name(name)
    solution = model.evaluate({0: 0, 1: 0})
    samples = model.evaluate_samples({7: {0: 0, 1: 0}})
    roots: tuple[Instance, ParametricInstance, Solution, SampleSet] = (
        model,
        model.as_parametric_instance(),
        solution,
        samples,
    )
    for root in roots:
        actual = root.constraint_names
        assert_type(actual, set[str])
        assert actual == set(names)
        actual.clear()
        assert root.constraint_names == set(names)  # A fresh independent set.
    for name in names:
        assert model.extract_constraints(name)
        assert model.as_parametric_instance().extract_constraints(name)
        assert solution.extract_constraints(name)
        assert samples.extract_constraints(name, 7)

    old_names = model.constraint_names
    model.constraints[ConstraintID(5)].set_name("renamed")
    assert model.constraint_names == {"renamed", "", "one_hot", "sos1"}
    assert (
        old_names == solution.constraint_names == samples.constraint_names == set(names)
    )


def test_constraint_names_deduplicate_without_validating_subscripts():
    model = label_model(make_model(Instance))
    for _, id_type, _ in FAMILIES:
        model.constraints[id_type(5)].set_subscripts([])
    solution = model.evaluate({0: 0, 1: 0})
    samples = model.evaluate_samples({7: {0: 0, 1: 0}})
    for root in [model, model.as_parametric_instance(), solution, samples]:
        assert root.constraint_names == {"c"}
    for root in [model, model.as_parametric_instance(), solution]:
        with pytest.raises(ValueError, match="Duplicate subscript"):
            root.extract_constraints("c")
    with pytest.raises(ValueError, match="Duplicate subscript"):
        samples.extract_constraints("c", 7)


def test_constraint_name_scope_tracks_removal_and_retained_results():
    model = make_model(Instance)
    for name, id_type, _ in FAMILIES:
        model.constraints[id_type(5)].set_name(name)
    remove_all(model)
    for row in model.constraints.values():
        assert isinstance(row, AttachedConstraint)
        row.set_name("generated")
    assert model.constraint_names == {"generated"}
    parametric = model.as_parametric_instance()
    assert parametric.constraint_names == {"generated"}
    solution = model.evaluate({0: 0, 1: 0})
    samples = model.evaluate_samples({7: {0: 0, 1: 0}})
    expected = {"generated", *(name for name, _, _ in FAMILIES)}
    assert solution.constraint_names == expected
    assert samples.constraint_names == expected
    assert samples.get(7).constraint_names == expected
    assert (
        type(solution).from_v2_bytes(solution.to_v2_bytes()).constraint_names
        == expected
    )
    assert (
        type(samples).from_v2_bytes(samples.to_v2_bytes()).constraint_names == expected
    )


def test_parametric_variable_names_exclude_parameters_and_track_label_edits():
    model = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=0,
        decision_variables=[
            DecisionVariable.binary(0, name="x"),
            DecisionVariable.binary(1, name="x"),
            DecisionVariable.binary(2, name=""),
            DecisionVariable.binary(3),
        ],
        parameters=[Parameter(100, name="parameter_only")],
        constraints={},
    )
    names = model.decision_variable_names
    assert_type(names, set[str])
    assert names == {"x", ""}
    names.add("external")
    assert model.decision_variable_names == {"x", ""}
    model.attached_decision_variable(0).set_name("renamed")
    assert model.decision_variable_names == {"x", "", "renamed"}
    assert (
        model.with_parameters({100: 1}).decision_variable_names
        == model.decision_variable_names
    )
    assert model.constraint_names == set()
    empty = Instance.minimize()
    assert empty.constraint_names == set()
    assert empty.as_parametric_instance().decision_variable_names == set()
    assert empty.evaluate({}).constraint_names == set()
    assert empty.evaluate_samples({7: {}}).constraint_names == set()
