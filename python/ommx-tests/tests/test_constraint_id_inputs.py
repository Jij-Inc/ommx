"""Promote integers only when the API argument fixes a constraint family."""

import pytest
from typing import Any
from types import MappingProxyType
from typing_extensions import assert_type

from ommx import (
    AttachedConstraint,
    AttachedIndicatorConstraint,
    AttachedOneHotConstraint,
    AttachedSos1Constraint,
    Constraint,
    ConstraintID,
    DecisionVariable,
    Equality,
    IndicatorConstraintID,
    Instance,
    InstanceClassMismatch,
    OneHotConstraint,
    OneHotConstraintID,
    ParametricInstance,
    PolynomialRequirement,
    Sense,
    Sos1BigMPromotionRequest,
    Sos1BigMSelectorClaim,
    Sos1Constraint,
    Sos1ConstraintID,
)

FAMILIES = [
    ("constraints", ConstraintID),
    ("indicator_constraints", IndicatorConstraintID),
    ("one_hot_constraints", OneHotConstraintID),
    ("sos1_constraints", Sos1ConstraintID),
]


def components() -> dict[str, Any]:
    x, y = DecisionVariable.binary(0), DecisionVariable.binary(1)
    return dict(
        sense=Sense.Minimize,
        objective=x + y,
        decision_variables=[x, y],
        constraints={7: x + y == 1},
        indicator_constraints={7: (y == 1).with_indicator(x)},
        one_hot_constraints={7: OneHotConstraint(variables=[x, y])},
        sos1_constraints={7: Sos1Constraint(variables=[x, y])},
    )


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
def test_integer_and_typed_keys_promote_to_the_same_family(host):
    kwargs = components()
    for name, id_type in FAMILIES:
        kwargs[name][id_type(42)] = next(iter(kwargs[name].values()))
    if host is ParametricInstance:
        kwargs["parameters"] = []
    instance = host.from_components(**kwargs)
    for name, id_type in FAMILIES:
        rows = getattr(instance, name)
        assert set(rows) == {id_type(7), id_type(42)}
        assert all(type(id_) is id_type for id_ in rows)
        assert rows[id_type(7)].constraint_id == id_type(7)
        assert 7 not in rows  # Returned dictionaries retain normal Python equality.
    assert (
        instance.get_constraint_by_id(7).equality
        == instance.get_constraint_by_id(ConstraintID(7)).equality
    )


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
@pytest.mark.parametrize("name,id_type", FAMILIES)
def test_mapping_keys_reject_every_other_id_family(host, name, id_type):
    for _, wrong_type in FAMILIES:
        if wrong_type is id_type:
            continue
        kwargs = components()
        row = kwargs[name].pop(7)
        kwargs[name][wrong_type(7)] = row
        if host is ParametricInstance:
            kwargs["parameters"] = []
        with pytest.raises(TypeError, match=f"Expected int or {id_type.__name__}"):
            host.from_components(**kwargs)


@pytest.mark.parametrize("host", [Instance, ParametricInstance])
@pytest.mark.parametrize("name,id_type", FAMILIES)
def test_mapping_rejects_duplicate_ids_after_promotion(host, name, id_type):
    kwargs = components()
    kwargs[name][id_type(7)] = kwargs[name][7]
    if host is ParametricInstance:
        kwargs["parameters"] = []
    with pytest.raises(
        ValueError, match="Duplicate constraint ID after integer promotion"
    ):
        host.from_components(**kwargs)


def test_regular_integer_lookup_mutation_and_solution_queries():
    instance = Instance.from_components(**components())
    instance.relax_constraint(7, "manual")
    assert set(instance.removed_constraints) == {ConstraintID(7)}
    assert instance.get_removed_constraint_by_id(7).removed_reason == "manual"
    instance.restore_constraint(7)
    assert set(instance.constraints) == {ConstraintID(7)}
    instance.tighten_bounds_simultaneously_once_using_constraints({7, ConstraintID(7)})
    solution = instance.evaluate({0: 1, 1: 0})
    assert solution.get_constraint_value(7) == 0
    solution.set_dual_variable(7, 3.0)
    assert solution.get_dual_variable(ConstraintID(7)) == 3
    solution.set_dual_variable(ConstraintID(7), 5)
    assert solution.get_dual_variable(7) == 5
    assert set(solution.get_constraints(kind="regular")) == {ConstraintID(7)}


def test_special_integer_mutations_and_typed_transformation_results():
    instance = Instance.from_components(**components())
    instance.relax_indicator_constraint(7, "manual")
    assert set(instance.removed_indicator_constraints) == {IndicatorConstraintID(7)}
    instance.restore_indicator_constraint(7)
    regular_id = instance.convert_one_hot_to_constraint(7)
    assert_type(regular_id, ConstraintID)
    assert type(regular_id) is ConstraintID
    for id_ in instance.convert_sos1_to_constraints(7):
        assert type(id_) is ConstraintID
    for id_ in instance.convert_indicator_to_constraint(7):
        assert type(id_) is ConstraintID
    assert instance.get_constraint_by_id(regular_id).provenance[
        0
    ].original_id == OneHotConstraintID(7)


@pytest.mark.parametrize(
    "method,id_type",
    [
        ("convert_one_hot_to_constraint", OneHotConstraintID),
        ("convert_sos1_to_constraints", Sos1ConstraintID),
        ("convert_indicator_to_constraint", IndicatorConstraintID),
    ],
)
def test_special_mutations_reject_other_families_atomically(method, id_type):
    instance = Instance.from_components(**components())
    before = instance.to_v2_bytes()
    for _, wrong_type in FAMILIES:
        if wrong_type is id_type:
            continue
        with pytest.raises(TypeError, match=f"Expected int or {id_type.__name__}"):
            getattr(instance, method)(wrong_type(7))
        assert instance.to_v2_bytes() == before


def test_nested_promotion_request_and_optional_links_return_typed_ids():
    claim = Sos1BigMSelectorClaim.fresh(0, upper_link=7, lower_link=ConstraintID(8))
    assert_type(claim.upper_link, ConstraintID | None)
    assert claim.upper_link == ConstraintID(7)
    assert claim.lower_link == ConstraintID(8)
    request = Sos1BigMPromotionRequest({7: {0: claim}, ConstraintID(42): {}})
    assert set(request.selector_claims) == {ConstraintID(7), ConstraintID(42)}
    assert request.selector_claims[ConstraintID(7)][0].upper_link == ConstraintID(7)
    for wrong in [IndicatorConstraintID(7), OneHotConstraintID(7), Sos1ConstraintID(7)]:
        with pytest.raises(TypeError, match="Expected int or ConstraintID"):
            Sos1BigMSelectorClaim.fresh(0, upper_link=wrong)
        with pytest.raises(TypeError, match="Expected int or ConstraintID"):
            Sos1BigMPromotionRequest({wrong: {}})


class IndexOnly:
    def __index__(self):
        return 7


@pytest.mark.parametrize("value", [True, False, 7.0, "7", IndexOnly()])
def test_input_promotion_requires_an_integer(value):
    instance = Instance.from_components(**components())
    before = instance.to_v2_bytes()
    with pytest.raises(TypeError, match="Expected int or ConstraintID"):
        instance.relax_constraint(value, "manual")
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("value", [-1, 2**64])
def test_input_promotion_checks_unsigned_64_bit_range(value):
    instance = Instance.from_components(**components())
    before = instance.to_v2_bytes()
    with pytest.raises(OverflowError):
        instance.relax_constraint(value, "manual")
    assert instance.to_v2_bytes() == before


def test_static_input_unions_do_not_widen_output_types():
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={7: x == 1, ConstraintID(42): x <= 1},
        indicator_constraints={7: (x == 1).with_indicator(x)},
        one_hot_constraints={7: OneHotConstraint(variables=[x])},
        sos1_constraints={7: Sos1Constraint(variables=[x])},
    )
    assert_type(instance.constraints, dict[ConstraintID, AttachedConstraint])
    assert_type(
        instance.indicator_constraints,
        dict[IndicatorConstraintID, AttachedIndicatorConstraint],
    )
    assert_type(
        instance.one_hot_constraints, dict[OneHotConstraintID, AttachedOneHotConstraint]
    )
    assert_type(
        instance.sos1_constraints, dict[Sos1ConstraintID, AttachedSos1Constraint]
    )


def test_homogeneous_mapping_variables_are_valid_inputs():
    x = DecisionVariable.binary(0)
    integers: dict[int, Constraint] = {7: x == 1}
    typed: dict[ConstraintID, Constraint] = {ConstraintID(7): x == 1}
    mixed: dict[int | ConstraintID, Constraint] = {7: x == 1, ConstraintID(8): x <= 1}
    for rows in (integers, typed, mixed, MappingProxyType(integers)):
        instance = Instance.from_components(
            sense=Sense.Minimize,
            objective=x,
            decision_variables=[x],
            constraints=rows,
        )
        assert_type(instance.constraints, dict[ConstraintID, AttachedConstraint])
        assert instance.get_constraint_by_id(7).equality == (x == 1).equality


@pytest.mark.parametrize(
    "variant,id_type,extra",
    [
        (
            InstanceClassMismatch.RegularConstraintFunctionNotPolynomial,
            ConstraintID,
            {"relation": Equality.EqualToZero},
        ),
        (
            InstanceClassMismatch.IndicatorConstraintsNotAllowed,
            IndicatorConstraintID,
            {},
        ),
        (InstanceClassMismatch.OneHotConstraintsNotAllowed, OneHotConstraintID, {}),
        (InstanceClassMismatch.Sos1ConstraintsNotAllowed, Sos1ConstraintID, {}),
    ],
)
def test_diagnostic_constructors_promote_ids(variant, id_type, extra):
    mismatch = variant(constraint_ids={7, id_type(42)}, **extra)
    assert mismatch.constraint_ids == {id_type(7), id_type(42)}
    for _, wrong_type in FAMILIES:
        if wrong_type is not id_type:
            with pytest.raises(TypeError, match=f"Expected int or {id_type.__name__}"):
                variant(constraint_ids={wrong_type(7)}, **extra)


def test_diagnostic_degree_map_promotes_keys_and_preserves_output_type():
    degrees: dict[int, int] = {7: 2}
    mismatch = InstanceClassMismatch.RegularConstraintDegreeExceedsBound(
        relation=Equality.EqualToZero,
        actual_degrees=degrees,
        bound=PolynomialRequirement.at_most(1),
    )
    assert_type(mismatch.actual_degrees, dict[ConstraintID, int])
    assert mismatch.actual_degrees == {ConstraintID(7): 2}
    with pytest.raises(
        ValueError, match="Duplicate constraint ID after integer promotion"
    ):
        InstanceClassMismatch.RegularConstraintDegreeExceedsBound(
            relation=Equality.EqualToZero,
            actual_degrees={7: 2, ConstraintID(7): 3},
            bound=PolynomialRequirement.at_most(1),
        )


def test_diagnostic_set_input_preserves_typed_output():
    ids: set[int] = {7}
    typed_ids: set[Sos1ConstraintID] = {Sos1ConstraintID(42)}
    mixed_ids: set[int | Sos1ConstraintID] = {7, Sos1ConstraintID(42)}
    for keys in (ids, typed_ids, mixed_ids, frozenset(ids)):
        mismatch = InstanceClassMismatch.Sos1ConstraintsNotAllowed(keys)
        assert_type(mismatch.constraint_ids, set[Sos1ConstraintID])
        assert all(type(id_) is Sos1ConstraintID for id_ in mismatch.constraint_ids)
