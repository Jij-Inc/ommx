from __future__ import annotations

from typing import Any, cast

import pytest

from ommx import (
    DecisionVariable,
    DegreeBound,
    Equality,
    IndicatorConstraint,
    Instance,
    InstanceClass,
    InstanceClassClause,
    InstanceClassMembershipReport,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx.adapter import (
    AdapterApplicabilityReport,
    AdapterNotApplicableError,
    AdapterPreconditionViolation,
    ConstraintRef,
    SolverAdapter,
)


def clause(
    label: str,
    *,
    allowed_variable_kinds: set[Kind],
    objective_degree_bound: DegreeBound,
    allowed_senses: set[Sense] | None = None,
    regular_constraint_degree_bounds: dict[Equality, DegreeBound] | None = None,
    indicator_constraint_degree_bounds: dict[Equality, DegreeBound] | None = None,
    allows_one_hot: bool = False,
    allows_sos1: bool = False,
) -> InstanceClassClause:
    return InstanceClassClause(
        label=label,
        allowed_variable_kinds=allowed_variable_kinds,
        objective_degree_bound=objective_degree_bound,
        allowed_senses=({Sense.Minimize} if allowed_senses is None else allowed_senses),
        regular_constraint_degree_bounds=regular_constraint_degree_bounds,
        indicator_constraint_degree_bounds=indicator_constraint_degree_bounds,
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
                objective_degree_bound=DegreeBound.at_most(1),
                allows_one_hot=allows_one_hot,
            )
        ]
    )


def test_degree_bound_and_clause_declaration() -> None:
    linear = DegreeBound.at_most(1)
    assert linear.maximum == 1
    assert linear.includes(0)
    assert linear.includes(1)
    assert not linear.includes(2)
    assert DegreeBound.unbounded().maximum is None
    assert DegreeBound.unbounded().includes(10_000)

    declared = clause(
        "linear",
        allowed_variable_kinds={Kind.Binary, Kind.Integer},
        objective_degree_bound=linear,
        allowed_senses={Sense.Minimize, Sense.Maximize},
        regular_constraint_degree_bounds={
            Equality.EqualToZero: linear,
            Equality.LessThanOrEqualToZero: linear,
        },
        allows_one_hot=True,
    )
    assert declared.label == "linear"
    assert declared.allowed_variable_kinds == {Kind.Binary, Kind.Integer}
    assert declared.objective_degree_bound == linear
    assert declared.allowed_senses == {Sense.Minimize, Sense.Maximize}
    assert declared.regular_constraint_degree_bounds == {
        Equality.EqualToZero: linear,
        Equality.LessThanOrEqualToZero: linear,
    }
    assert declared.indicator_constraint_degree_bounds == {}
    assert declared.allows_one_hot
    assert not declared.allows_sos1

    assert ConstraintRef("regular", 1) != ConstraintRef("indicator", 1)


def test_empty_classes_duplicate_labels_and_membership_is_side_effect_free() -> None:
    x = DecisionVariable.binary(1)
    instance = instance_with_objective(x, x)
    before = instance.to_v2_bytes()

    assert not InstanceClass([]).contains(instance)
    empty_clause = clause(
        "empty",
        allowed_variable_kinds={Kind.Binary},
        objective_degree_bound=DegreeBound.unbounded(),
        allowed_senses=set(),
    )
    assert not InstanceClass([empty_clause]).contains(instance)

    duplicate = clause(
        "duplicate",
        allowed_variable_kinds={Kind.Binary},
        objective_degree_bound=DegreeBound.at_most(1),
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
        Equality.EqualToZero: DegreeBound.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
    }
    input_class = InstanceClass(
        [
            clause(
                "milp",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_degree_bound=DegreeBound.at_most(1),
                regular_constraint_degree_bounds=relations,
            ),
            clause(
                "continuous-qp",
                allowed_variable_kinds={Kind.Continuous},
                objective_degree_bound=DegreeBound.at_most(2),
                regular_constraint_degree_bounds=relations,
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
        objective_degree_bound=DegreeBound.at_most(1),
        allowed_senses={Sense.Minimize},
        regular_constraint_degree_bounds={Equality.EqualToZero: DegreeBound.at_most(1)},
        indicator_constraint_degree_bounds={
            Equality.EqualToZero: DegreeBound.at_most(1)
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
    assert objective.bound == DegreeBound.at_most(1)

    regular_degree = mismatches[2]
    assert isinstance(
        regular_degree, InstanceClassMismatch.RegularConstraintDegreeExceedsBound
    )
    assert regular_degree.relation == Equality.EqualToZero
    assert regular_degree.actual_degrees == {10: 2}
    assert regular_degree.bound == DegreeBound.at_most(1)

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
        objective_degree_bound=DegreeBound.unbounded(),
        allowed_senses={Sense.Minimize, Sense.Maximize},
        regular_constraint_degree_bounds={
            Equality.EqualToZero: DegreeBound.unbounded(),
            Equality.LessThanOrEqualToZero: DegreeBound.unbounded(),
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
                objective_degree_bound=DegreeBound.at_most(1),
                regular_constraint_degree_bounds={
                    Equality.EqualToZero: DegreeBound.at_most(1)
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


def test_solver_adapter_has_no_implicit_input_transformation_hook() -> None:
    assert "__init__" not in SolverAdapter.__dict__


def test_solver_adapter_layers_preconditions_and_preserves_the_caller() -> None:
    x = DecisionVariable.binary(1)
    instance = instance_with_objective(x, x)
    before = instance.to_v2_bytes()
    violation = AdapterPreconditionViolation(
        condition="backend_limit",
        description="backend accepts no variables in this test",
        variable_ids=frozenset({1}),
        actual=1,
        limit=0,
    )

    class Adapter(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class()
        calls = 0

        @classmethod
        def _check_preconditions(cls, ommx_instance, input_membership):
            cls.calls += 1
            assert input_membership.is_member
            assert ommx_instance.to_v2_bytes() == before
            return (violation,)

    report = Adapter.check_applicability(instance)
    assert not report.is_applicable
    assert report.input_membership.is_member
    assert report.preconditions_checked
    assert report.precondition_violations == (violation,)
    assert report.adapter.endswith(".Adapter")
    assert Adapter.calls == 1
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterNotApplicableError) as exc_info:
        Adapter.require_applicable(instance)
    assert exc_info.value.report.precondition_violations == (violation,)


def test_membership_failure_skips_preconditions_and_missing_declaration_is_error() -> (
    None
):
    y = DecisionVariable.continuous(1)
    outside_input_class = instance_with_objective(y, y * y)

    class Adapter(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class()
        calls = 0

        @classmethod
        def _check_preconditions(cls, ommx_instance, input_membership):
            cls.calls += 1
            return ()

    report = Adapter.check_applicability(outside_input_class)
    assert not report.is_applicable
    assert not report.input_membership.is_member
    assert not report.preconditions_checked
    assert report.precondition_violations == ()
    assert Adapter.calls == 0

    class MissingDeclaration(SolverAdapter):
        pass

    with pytest.raises(TypeError, match="must declare INPUT_CLASS"):
        MissingDeclaration.check_applicability(outside_input_class)


def test_applicability_report_rejects_inconsistent_states() -> None:
    x = DecisionVariable.binary(1)
    member = binary_linear_input_class().check_membership(instance_with_objective(x, x))
    y = DecisionVariable.continuous(2)
    nonmember = binary_linear_input_class().check_membership(
        instance_with_objective(y, y)
    )
    violation = AdapterPreconditionViolation(
        condition="backend_limit",
        description="backend limit was exceeded",
    )

    with pytest.raises(ValueError, match="exactly when input membership holds"):
        AdapterApplicabilityReport("Adapter", member, False, ())
    with pytest.raises(ValueError, match="exactly when input membership holds"):
        AdapterApplicabilityReport("Adapter", nonmember, True, ())
    with pytest.raises(ValueError, match="require adapter preconditions"):
        AdapterApplicabilityReport("Adapter", nonmember, False, (violation,))


def test_precondition_hook_is_isolated_and_validated() -> None:
    x = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
        one_hot_constraints={30: OneHotConstraint(variables=[x])},
    )
    before = instance.to_v2_bytes()

    class MutatingHook(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class(allows_one_hot=True)

        @classmethod
        def _check_preconditions(
            cls,
            ommx_instance: Instance,
            input_membership: InstanceClassMembershipReport,
        ) -> tuple[AdapterPreconditionViolation, ...]:
            ommx_instance.lower_special_constraints({SpecialConstraintKind.OneHot})
            return ()

    assert MutatingHook.check_applicability(instance).is_applicable
    assert instance.to_v2_bytes() == before
    assert set(instance.one_hot_constraints) == {30}

    class InvalidHook(SolverAdapter):
        INPUT_CLASS = binary_linear_input_class(allows_one_hot=True)

        @classmethod
        def _check_preconditions(
            cls,
            ommx_instance: Instance,
            input_membership: InstanceClassMembershipReport,
        ) -> tuple[AdapterPreconditionViolation, ...]:
            return cast(tuple[AdapterPreconditionViolation, ...], ("not a violation",))

    with pytest.raises(TypeError, match="AdapterPreconditionViolation"):
        InvalidHook.check_applicability(instance)
