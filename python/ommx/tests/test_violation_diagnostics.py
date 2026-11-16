"""Typed diagnostic snapshots preserve the result owner's evaluation contract."""

from collections.abc import Mapping, KeysView, ValuesView, ItemsView
from typing import Any, cast

import pytest

from ommx import (
    ConstraintID,
    ConstraintIDBase,
    ConstraintKind,
    ConstraintRole,
    DecisionVariable,
    IndicatorConstraintID,
    Instance,
    OneHotConstraint,
    OneHotConstraintID,
    OneHotViolationDiagnostics,
    OneHotMissingSelection,
    OneHotMultipleSelections,
    OneHotValueErrors,
    RegularViolationDiagnostics,
    IndicatorViolationDiagnostics,
    Sos1Constraint,
    Sos1ConstraintID,
    Sos1ViolationDiagnostics,
    Sense,
    Solution,
    SampleSet,
    ViolationDiagnostics,
    ViolationDiagnosticsBase,
    ViolationDiagnosticsFamilyView,
)

FAMILIES = [
    (ConstraintKind.Regular, ConstraintID, RegularViolationDiagnostics, 0),
    (
        ConstraintKind.Indicator,
        IndicatorConstraintID,
        IndicatorViolationDiagnostics,
        10,
    ),
    (ConstraintKind.OneHot, OneHotConstraintID, OneHotViolationDiagnostics, 20),
    (ConstraintKind.Sos1, Sos1ConstraintID, Sos1ViolationDiagnostics, 30),
]


def model(copies=1, *, indicator=True):
    x, y, z = [DecisionVariable.binary(i) for i in range(3)]
    return Instance.from_components(
        decision_variables=[x, y, z],
        objective=0,
        sense=Sense.Minimize,
        active_constraints={i: x + y == 0 for i in range(copies)},
        active_indicator_constraints={
            10 + i: (x + y == 0).with_indicator(z) for i in range(copies)
        }
        if indicator
        else {},
        active_one_hot_constraints={
            20 + i: OneHotConstraint(variables=[x, y, z]) for i in range(copies)
        },
        active_sos1_constraints={
            30 + i: Sos1Constraint(variables=[x, y, z]) for i in range(copies)
        },
    )


def test_mapping_and_family_protocols():
    solution = model().evaluate({0: 1, 1: 1, 2: 1})
    diagnostics = solution.violation_diagnostics()
    assert isinstance(diagnostics, (ViolationDiagnostics, Mapping))
    expected = [
        ConstraintID(0),
        IndicatorConstraintID(10),
        OneHotConstraintID(20),
        Sos1ConstraintID(30),
    ]
    assert list(diagnostics) == expected
    assert len(diagnostics) == 4
    assert isinstance(diagnostics.keys(), KeysView)
    assert isinstance(diagnostics.values(), ValuesView)
    assert isinstance(diagnostics.items(), ItemsView)
    assert list(diagnostics.keys()) == expected
    assert list(diagnostics.items()) == list(zip(expected, diagnostics.values()))
    assert diagnostics == dict(diagnostics)
    assert dict(diagnostics) == diagnostics
    assert diagnostics == solution.violation_diagnostics()
    assert diagnostics != {}
    assert diagnostics != []
    assert diagnostics == diagnostics
    default = object()
    for kind, id_type, value_type, number in FAMILIES:
        key, missing = id_type(number), id_type(999)
        assert isinstance(diagnostics[key], value_type)
        assert isinstance(diagnostics[key], ViolationDiagnosticsBase)
        assert diagnostics.get(key, default) == diagnostics[key]
        assert diagnostics.get(missing) is None
        assert diagnostics.get(missing, default) is default
        assert missing not in diagnostics
        with pytest.raises(KeyError) as error:
            diagnostics[missing]
        assert error.value.args == (missing,)
        family = diagnostics.of_kind(kind)
        assert isinstance(family, (ViolationDiagnosticsFamilyView, Mapping))
        assert list(family) == [key]
        assert family == {key: diagnostics[key]}
        assert family[number] == family[key] == diagnostics[key]
        assert family.get(number, default) == diagnostics[key]
        assert family.get(999, default) is default
        assert family.get(missing) is None
        assert number in family and key in family
        assert 999 not in family
        assert list(family.keys()) == [key]
        assert list(family.values()) == [diagnostics[key]]
        assert list(family.items()) == [(key, diagnostics[key])]
        with pytest.raises(KeyError) as error:
            family[999]
        assert error.value.args == (missing,)
        wrong = (
            OneHotConstraintID(number)
            if id_type is not OneHotConstraintID
            else ConstraintID(number)
        )
        for operation in (
            lambda: family[wrong],
            lambda: family.get(wrong),
            lambda: wrong in family,
        ):
            with pytest.raises(TypeError):
                operation()
    for key in cast(list[Any], [0, True, "0", None, ConstraintIDBase()]):
        for operation in (
            lambda: diagnostics[key],
            lambda: diagnostics.get(key),
            lambda: key in diagnostics,
        ):
            with pytest.raises(TypeError):
                operation()
    for invalid in cast(list[Any], [True, -1, 2**64, "0"]):
        with pytest.raises((TypeError, OverflowError)):
            diagnostics.of_kind(ConstraintKind.Regular)[invalid]
    mutable: Any = diagnostics
    with pytest.raises(TypeError):
        mutable[ConstraintID(0)] = None
    with pytest.raises(TypeError):
        del mutable[ConstraintID(0)]


@pytest.mark.parametrize(
    "state, variant, values",
    [
        ({0: 0, 1: 0, 2: 0}, OneHotMissingSelection, {0: 0, 1: 0, 2: 0}),
        ({0: 1, 1: 0.9375, 2: -0.5}, OneHotMultipleSelections, {0: 1, 1: 1, 2: -0.5}),
        ({0: 0.875, 1: 0.25, 2: 0}, OneHotValueErrors, {0: 0.875, 1: 0.25, 2: 0}),
    ],
)
def test_one_hot_variants_and_snapshot_independence(state, variant, values):
    atol = 0.125 if variant is OneHotMultipleSelections else 0.0625
    solution = model(indicator=False).evaluate(state, atol=atol)
    diagnostics = solution.violation_diagnostics()
    detail = diagnostics[OneHotConstraintID(20)]
    assert isinstance(detail, variant)
    assert isinstance(detail, OneHotViolationDiagnostics)
    assert isinstance(detail, ViolationDiagnosticsBase)
    assert detail.values == values
    detail.values.clear()
    assert detail.values == values
    assert diagnostics == solution.violation_diagnostics()
    samples = model(indicator=False).evaluate_samples({7: state}, atol=atol)
    assert (
        Solution.from_v2_bytes(solution.to_v2_bytes()).violation_diagnostics()
        == diagnostics
    )
    for restored in (samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())):
        assert restored.violation_diagnostics(7) == diagnostics
        assert restored.get(7).violation_diagnostics() == diagnostics

    with pytest.raises(AttributeError):
        cast(Any, detail).values = {}
    if isinstance(detail, OneHotMultipleSelections):
        assert detail.selected_variables == {0, 1}
        detail.selected_variables.clear()
        assert detail.selected_variables == {0, 1}
    if isinstance(detail, OneHotValueErrors):
        assert detail.target_variable == 0
        assert detail.errors == {0: 0.125, 1: 0.25}
        detail.errors.clear()
        assert detail.errors == {0: 0.125, 1: 0.25}
    view = diagnostics.of_kind(ConstraintKind.OneHot)
    del solution, diagnostics
    assert view[20] == detail


def test_one_hot_enum_style_aliases_and_pattern_matching():
    assert OneHotViolationDiagnostics.MissingSelection is OneHotMissingSelection
    assert OneHotViolationDiagnostics.MultipleSelections is OneHotMultipleSelections
    assert OneHotViolationDiagnostics.ValueErrors is OneHotValueErrors
    for state, expected in [
        ({0: 0, 1: 0, 2: 0}, "missing"),
        ({0: 1, 1: 1, 2: 0}, "multiple"),
        ({0: 0.5, 1: 0.5, 2: 0}, "errors"),
    ]:
        detail = model().evaluate(state).violation_diagnostics()[OneHotConstraintID(20)]
        match detail:
            case OneHotViolationDiagnostics.MissingSelection(values=values):
                assert expected == "missing"
            case OneHotViolationDiagnostics.MultipleSelections(values=values):
                assert expected == "multiple"
            case OneHotViolationDiagnostics.ValueErrors(
                values=values, target_variable=target
            ):
                assert expected == "errors" and target == 1
            case _:
                pytest.fail("unsupported variant")
        assert values == state


def test_sos1_signed_values_ties_and_inclusive_tolerance():
    solution = model(indicator=False).evaluate(
        {0: -0.875, 1: 0.875, 2: 0.25}, atol=0.125
    )
    detail = solution.violation_diagnostics()[Sos1ConstraintID(30)]
    assert detail.values == {0: -0.875, 2: 0.25}  # equal magnitudes retain larger ID
    detail.values.clear()
    assert detail.values == {0: -0.875, 2: 0.25}
    for state in ({0: 1, 1: 0.125, 2: -0.125}, {0: 0, 1: 0, 2: 0}):
        result = model().evaluate(state, atol=0.125)
        assert Sos1ConstraintID(30) not in result.violation_diagnostics()
    result = model().evaluate({0: 0.875, 1: 0.25, 2: 0}, atol=0.125)
    before = result.violation_diagnostics()
    assert result.constraints[OneHotConstraintID(20)].is_feasible(atol=0.25)
    assert result.violation_diagnostics() == before
    one_hot = before[OneHotConstraintID(20)]
    assert isinstance(one_hot, OneHotValueErrors)
    assert one_hot.errors == {1: 0.25}


def test_role_filtering_samples_and_wire_round_trips():
    instance = model(copies=3)
    for offset, skip in [(1, False), (2, True)]:
        for method, base in [
            (instance.deactivate_constraint, 0),
            (instance.deactivate_indicator_constraint, 10),
            (instance.deactivate_one_hot_constraint, 20),
            (instance.deactivate_sos1_constraint, 30),
        ]:
            method(base + offset, "retained", skip_feasibility_check=skip)
    states = {2: {0: 1, 1: 1, 2: 1}, 7: {0: 1, 1: 1, 2: 1}, 13: {0: 0, 1: 0, 2: 1}}
    samples = instance.evaluate_samples(states)
    for collection in (samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())):
        for sample_id, state in states.items():
            solution = instance.evaluate(state)
            for result in (
                solution,
                collection.get(sample_id),
                Solution.from_v2_bytes(solution.to_v2_bytes()),
            ):
                for role, count in [
                    (ConstraintRole.ACTIVE, 1),
                    (ConstraintRole.FEASIBILITY_CHECK, 2),
                    (ConstraintRole.EVALUATION_ONLY, 3),
                ]:
                    diagnostics = result.violation_diagnostics(min_role=role)
                    assert diagnostics == collection.violation_diagnostics(
                        sample_id, min_role=role
                    )
                    expected = (
                        {
                            id_type(base + i)
                            for _, id_type, _, base in FAMILIES
                            for i in range(count)
                        }
                        if sample_id != 13
                        else set()
                    )
                    assert set(diagnostics) == expected
    for sample_id in (999,):
        with pytest.raises(KeyError):
            samples.violation_diagnostics(sample_id)
    empty = Instance.minimize().evaluate_samples({7: {}})
    assert empty.violation_diagnostics(7) == {}
    with pytest.raises(KeyError):
        empty.violation_diagnostics(999)
    with pytest.raises(KeyError):
        Instance.minimize().evaluate_samples({}).violation_diagnostics(7)


def test_empty_diagnostics_do_not_imply_variable_feasibility():
    instance = Instance.minimize()
    x = instance.new_binary("x")
    instance.objective = x
    result = instance.evaluate({x.id: 0.5})
    assert not result.feasible
    assert result.violation_diagnostics() == {}


def test_sos1_tolerance_boundary_on_uncanonicalized_continuous_values():
    import math

    instance = Instance.minimize()
    xs = [instance.new_continuous(f"x{i}", lower=-2, upper=2) for i in range(3)]
    attached = instance.add_sos1_constraint(Sos1Constraint(variables=xs))
    for value, violated in [(0.125, False), (math.nextafter(0.125, 1), True)]:
        state = {xs[0].id: 1, xs[1].id: value, xs[2].id: -value}
        solution = instance.evaluate(state, atol=0.125)
        sampled = instance.evaluate_samples({7: state}, atol=0.125)
        diagnostics = solution.violation_diagnostics()
        assert diagnostics == sampled.violation_diagnostics(7)
        assert (attached.constraint_id in diagnostics) == violated
        if violated:
            assert diagnostics[attached.constraint_id].values == {
                xs[1].id: value,
                xs[2].id: -value,
            }
