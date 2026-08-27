from __future__ import annotations

from typing import Any, cast

import pytest

from ommx import (
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    InstanceClass,
    InstanceClassClause,
    InstanceClassMembershipReport,
    InstanceClassMismatch,
    Kind,
    ObjectivePreparation,
    OneHotConstraint,
    PreparationPolicy,
    PolynomialRequirement,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx.adapter import (
    AdapterNotApplicableError,
    SolverAdapter,
)


def clause(
    label: str,
    *,
    allowed_variable_kinds: set[Kind],
    objective_polynomial_requirement: PolynomialRequirement,
    allowed_senses: set[Sense] | None = None,
    regular_constraint_polynomial_requirements: (
        dict[Equality, PolynomialRequirement] | None
    ) = None,
    indicator_body_polynomial_requirements: (
        dict[Equality, PolynomialRequirement] | None
    ) = None,
    allows_one_hot: bool = False,
    allows_sos1: bool = False,
) -> InstanceClassClause:
    return InstanceClassClause(
        label=label,
        allowed_variable_kinds=allowed_variable_kinds,
        objective_polynomial_requirement=objective_polynomial_requirement,
        allowed_senses=({Sense.Minimize} if allowed_senses is None else allowed_senses),
        regular_constraint_polynomial_requirements=(
            regular_constraint_polynomial_requirements
        ),
        indicator_body_polynomial_requirements=(indicator_body_polynomial_requirements),
        allows_one_hot=allows_one_hot,
        allows_sos1=allows_sos1,
    )


def instance_with_objective(variable: DecisionVariable, objective: Any) -> Instance:
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=objective,
        decision_variables=[variable],
        constraints={},
    )


def binary_linear_input_class(*, allows_one_hot: bool = False) -> InstanceClass:
    return InstanceClass(
        [
            clause(
                "binary-linear",
                allowed_variable_kinds={Kind.Binary},
                objective_polynomial_requirement=PolynomialRequirement.at_most(1),
                allows_one_hot=allows_one_hot,
            )
        ]
    )


def test_polynomial_requirement_and_clause_declaration() -> None:
    linear = PolynomialRequirement.at_most(1)
    assert linear.maximum_degree == 1
    assert linear.accepts_degree(0)
    assert linear.accepts_degree(1)
    assert not linear.accepts_degree(2)
    assert repr(linear) == "PolynomialRequirement.at_most(1)"

    any_degree = PolynomialRequirement.any_degree()
    assert any_degree.maximum_degree is None
    assert any_degree.accepts_degree(10_000)
    assert repr(any_degree) == "PolynomialRequirement.any_degree()"

    declared = clause(
        "linear",
        allowed_variable_kinds={Kind.Binary, Kind.Integer},
        objective_polynomial_requirement=linear,
        allowed_senses={Sense.Minimize, Sense.Maximize},
        regular_constraint_polynomial_requirements={
            Equality.EqualToZero: linear,
            Equality.LessThanOrEqualToZero: linear,
        },
        allows_one_hot=True,
    )
    assert declared.label == "linear"
    assert declared.allowed_variable_kinds == {Kind.Binary, Kind.Integer}
    assert declared.objective_polynomial_requirement == linear
    assert declared.allowed_senses == {Sense.Minimize, Sense.Maximize}
    assert declared.regular_constraint_polynomial_requirements == {
        Equality.EqualToZero: linear,
        Equality.LessThanOrEqualToZero: linear,
    }
    assert declared.indicator_body_polynomial_requirements == {}
    assert declared.allows_one_hot
    assert not declared.allows_sos1


def test_empty_classes_duplicate_labels_and_membership_is_side_effect_free() -> None:
    x = DecisionVariable.binary(1)
    instance = instance_with_objective(x, x)
    before = instance.to_v2_bytes()

    assert not InstanceClass([]).contains(instance)
    empty_clause = clause(
        "empty",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=PolynomialRequirement.any_degree(),
        allowed_senses=set(),
    )
    assert not InstanceClass([empty_clause]).contains(instance)

    duplicate = clause(
        "duplicate",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=PolynomialRequirement.at_most(1),
    )
    report = InstanceClass([duplicate, duplicate]).check_membership(instance)
    assert report.is_member
    assert report.matching_clauses == [(0, "duplicate"), (1, "duplicate")]
    assert instance.to_v2_bytes() == before

    one_clause = InstanceClass([duplicate])
    assert one_clause.union(InstanceClass([])).contains(instance)
    assert one_clause.contains(instance)


def test_complete_clauses_do_not_cross_combine_into_miqp_membership() -> None:
    relations = {
        Equality.EqualToZero: PolynomialRequirement.at_most(1),
        Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(1),
    }
    input_class = InstanceClass(
        [
            clause(
                "milp",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_polynomial_requirement=PolynomialRequirement.at_most(1),
                regular_constraint_polynomial_requirements=relations,
            ),
            clause(
                "continuous-qp",
                allowed_variable_kinds={Kind.Continuous},
                objective_polynomial_requirement=PolynomialRequirement.at_most(2),
                regular_constraint_polynomial_requirements=relations,
            ),
        ]
    )

    continuous = DecisionVariable.continuous(1)
    continuous_qp = input_class.check_membership(
        instance_with_objective(continuous, continuous * continuous)
    )
    assert continuous_qp.is_member
    assert continuous_qp.matching_clauses == [(1, "continuous-qp")]

    integer = DecisionVariable.integer(1)
    milp = input_class.check_membership(instance_with_objective(integer, integer))
    assert milp.is_member
    assert milp.matching_clauses == [(0, "milp")]

    miqp = input_class.check_membership(
        instance_with_objective(integer, integer * integer)
    )
    assert not miqp.is_member
    assert isinstance(
        miqp.clause_reports[0].mismatches[0],
        InstanceClassMismatch.ObjectiveDegreeExceedsBound,
    )
    assert isinstance(
        miqp.clause_reports[1].mismatches[0],
        InstanceClassMismatch.VariableKindNotAllowed,
    )


def test_mismatch_variants_preserve_all_structured_payloads() -> None:
    x = DecisionVariable.binary(1)
    y = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x * y,
        decision_variables=[x, y],
        constraints={10: y * y == 0, 11: y <= 0},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x,
                function=y * y,
                equality=Equality.EqualToZero,
            ),
            21: IndicatorConstraint(
                indicator_variable=x,
                function=y,
                equality=Equality.LessThanOrEqualToZero,
            ),
        },
        one_hot_constraints={30: OneHotConstraint(variables=[x])},
        sos1_constraints={40: Sos1Constraint(variables=[y])},
    )
    limited = clause(
        "limited",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=PolynomialRequirement.at_most(1),
        allowed_senses={Sense.Minimize},
        regular_constraint_polynomial_requirements={
            Equality.EqualToZero: PolynomialRequirement.at_most(1)
        },
        indicator_body_polynomial_requirements={
            Equality.EqualToZero: PolynomialRequirement.at_most(1)
        },
    )
    mismatches = (
        InstanceClass([limited]).check_membership(instance).clause_reports[0].mismatches
    )

    unsupported_kind = mismatches[0]
    assert isinstance(unsupported_kind, InstanceClassMismatch.VariableKindNotAllowed)
    assert unsupported_kind.kind == Kind.Continuous
    assert unsupported_kind.variable_ids == {2}
    assert unsupported_kind.allowed_kinds == {Kind.Binary}

    objective = mismatches[1]
    assert isinstance(objective, InstanceClassMismatch.ObjectiveDegreeExceedsBound)
    assert objective.actual_degree == 2
    assert objective.bound == PolynomialRequirement.at_most(1)

    regular_degree = mismatches[2]
    assert isinstance(
        regular_degree, InstanceClassMismatch.RegularConstraintDegreeExceedsBound
    )
    assert regular_degree.relation == Equality.EqualToZero
    assert regular_degree.actual_degrees == {10: 2}
    assert regular_degree.bound == PolynomialRequirement.at_most(1)

    regular_relation = mismatches[3]
    assert isinstance(
        regular_relation,
        InstanceClassMismatch.RegularConstraintRelationNotAllowed,
    )
    assert regular_relation.relation == Equality.LessThanOrEqualToZero
    assert regular_relation.constraint_ids == {11}
    assert regular_relation.allowed_relations == {Equality.EqualToZero}

    indicator_degree = mismatches[4]
    assert isinstance(
        indicator_degree, InstanceClassMismatch.IndicatorBodyDegreeExceedsBound
    )
    assert indicator_degree.actual_degrees == {20: 2}

    indicator_relation = mismatches[5]
    assert isinstance(
        indicator_relation,
        InstanceClassMismatch.IndicatorConstraintRelationNotAllowed,
    )
    assert indicator_relation.constraint_ids == {21}

    assert isinstance(mismatches[6], InstanceClassMismatch.OneHotConstraintsNotAllowed)
    assert mismatches[6].constraint_ids == {30}
    assert isinstance(mismatches[7], InstanceClassMismatch.Sos1ConstraintsNotAllowed)
    assert mismatches[7].constraint_ids == {40}

    unsupported_sense = mismatches[8]
    assert isinstance(unsupported_sense, InstanceClassMismatch.SenseNotAllowed)
    assert unsupported_sense.sense == Sense.Maximize
    assert unsupported_sense.allowed_senses == {Sense.Minimize}

    no_indicator_support = clause(
        "no-indicator-support",
        allowed_variable_kinds={Kind.Binary, Kind.Continuous},
        objective_polynomial_requirement=PolynomialRequirement.any_degree(),
        allowed_senses={Sense.Minimize, Sense.Maximize},
        regular_constraint_polynomial_requirements={
            Equality.EqualToZero: PolynomialRequirement.any_degree(),
            Equality.LessThanOrEqualToZero: PolynomialRequirement.any_degree(),
        },
        allows_one_hot=True,
        allows_sos1=True,
    )
    unsupported_indicator = next(
        mismatch
        for mismatch in InstanceClass([no_indicator_support])
        .check_membership(instance)
        .clause_reports[0]
        .mismatches
        if isinstance(mismatch, InstanceClassMismatch.IndicatorConstraintsNotAllowed)
    )
    assert unsupported_indicator.constraint_ids == {20, 21}


def test_non_polynomial_mismatches_preserve_function_location() -> None:
    x = DecisionVariable.binary(1)
    absolute_x = abs(Function(x))
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=absolute_x,
        decision_variables=[x],
        constraints={10: absolute_x == 0},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x,
                function=absolute_x,
                equality=Equality.EqualToZero,
            )
        },
    )
    polynomial_only = clause(
        "polynomial-only",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=PolynomialRequirement.any_degree(),
        regular_constraint_polynomial_requirements={
            Equality.EqualToZero: PolynomialRequirement.any_degree()
        },
        indicator_body_polynomial_requirements={
            Equality.EqualToZero: PolynomialRequirement.any_degree()
        },
    )

    mismatches = (
        InstanceClass([polynomial_only])
        .check_membership(instance)
        .clause_reports[0]
        .mismatches
    )

    assert isinstance(
        mismatches[0], InstanceClassMismatch.ObjectiveFunctionNotPolynomial
    )
    assert isinstance(
        mismatches[1],
        InstanceClassMismatch.RegularConstraintFunctionNotPolynomial,
    )
    assert mismatches[1].relation == Equality.EqualToZero
    assert mismatches[1].constraint_ids == {10}
    assert isinstance(
        mismatches[2], InstanceClassMismatch.IndicatorBodyFunctionNotPolynomial
    )
    assert mismatches[2].relation == Equality.EqualToZero
    assert mismatches[2].constraint_ids == {20}


def test_membership_is_recomputed_after_explicit_lowering() -> None:
    x = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
        one_hot_constraints={30: OneHotConstraint(variables=[x])},
    )
    prepared_input_class = InstanceClass(
        [
            clause(
                "binary-linear-equality",
                allowed_variable_kinds={Kind.Binary},
                objective_polynomial_requirement=PolynomialRequirement.at_most(1),
                regular_constraint_polynomial_requirements={
                    Equality.EqualToZero: PolynomialRequirement.at_most(1)
                },
            )
        ]
    )

    assert not prepared_input_class.contains(instance)
    assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
    assert instance.lower_special_constraints(set()) == set()
    assert set(instance.one_hot_constraints) == {30}

    lowered = instance.lower_special_constraints({SpecialConstraintKind.OneHot})
    assert lowered == {SpecialConstraintKind.OneHot}
    assert instance.active_special_constraint_kinds == set()
    assert prepared_input_class.contains(instance)


def test_solver_adapter_returns_a_fresh_empty_preparation_recommendation() -> None:
    first = SolverAdapter.recommended_preparation_policy()
    second = SolverAdapter.recommended_preparation_policy()

    assert first == PreparationPolicy()
    assert second == PreparationPolicy()
    assert first is not second

    first.objective = ObjectivePreparation(target=Sense.Minimize)
    assert second.objective is None


def test_solver_adapter_applicability_is_input_class_membership() -> None:
    x = DecisionVariable.binary(1)
    instance = instance_with_objective(x, x)
    before = instance.to_v2_bytes()

    class Adapter(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class()

    report = Adapter.check_applicability(instance)
    assert isinstance(report, InstanceClassMembershipReport)
    assert report.is_member
    assert instance.to_v2_bytes() == before
    assert Adapter.require_applicable(instance) == report


def test_membership_failure_and_invalid_declarations_are_errors() -> None:
    y = DecisionVariable.continuous(1)
    outside_input_class = instance_with_objective(y, y * y)

    class Adapter(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class()

    report = Adapter.check_applicability(outside_input_class)
    assert not report.is_member
    with pytest.raises(AdapterNotApplicableError) as exc_info:
        Adapter.require_applicable(outside_input_class)
    assert exc_info.value.report == report
    assert exc_info.value.adapter.endswith(".Adapter")

    class MissingDeclaration(SolverAdapter):
        pass

    class NoneDeclaration(SolverAdapter):
        INPUT_CLASS = cast(InstanceClass, None)

    for adapter in (MissingDeclaration, NoneDeclaration):
        with pytest.raises(TypeError, match="must declare INPUT_CLASS"):
            adapter.check_applicability(outside_input_class)
