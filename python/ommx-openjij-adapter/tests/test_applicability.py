from __future__ import annotations

from typing import Any

import pytest

from ommx import (
    DecisionVariable,
    DegreeBound,
    Equality,
    IndicatorConstraint,
    Instance,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import (
    AdapterNotApplicableError,
    AdapterPreconditionViolation,
    ConstraintRef,
    InfeasibleDetected,
)
from ommx_openjij_adapter import (
    OMMXOpenJijSAAdapter,
    OpenJijPreparation,
    OpenJijPreparationError,
    OpenJijPreparationReport,
)


def _instance_with_variable(
    variable: DecisionVariable,
    *,
    sense: Sense = Sense.Minimize,
) -> Instance:
    return Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        constraints={},
        sense=sense,
    )


def _assert_direct_rejection_does_not_mutate(
    instance: Instance,
) -> tuple[InstanceClassMismatch, ...]:
    before = instance.to_v2_bytes()

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert not report.is_applicable
    assert not report.input_membership.is_member
    assert not report.preconditions_checked
    assert report.precondition_violations == ()
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterNotApplicableError) as error:
        OMMXOpenJijSAAdapter(instance)
    assert error.value.report.adapter == report.adapter
    assert not error.value.report.is_applicable
    assert instance.to_v2_bytes() == before

    [clause_report] = report.input_membership.clause_reports
    return tuple(clause_report.mismatches)


def _check_preparation_without_mutation(
    instance: Instance,
    **kwargs: Any,
) -> OpenJijPreparationReport:
    before = instance.to_v2_bytes()
    report = OMMXOpenJijSAAdapter.check_preparation(instance, **kwargs)
    assert instance.to_v2_bytes() == before
    return report


def _violation_text(violation: AdapterPreconditionViolation) -> str:
    return f"{violation.condition} {violation.description}".lower()


def test_declares_binary_polynomial_input_class() -> None:
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
    assert input_class is not None

    [clause] = input_class.clauses
    assert clause.label == "openjij-binary-hubo"
    assert clause.allowed_variable_kinds == {Kind.Binary}
    assert clause.objective_degree_bound == DegreeBound.unbounded()
    assert clause.regular_constraint_degree_bounds == {}
    assert clause.indicator_constraint_degree_bounds == {}
    assert not clause.allows_one_hot
    assert not clause.allows_sos1
    assert clause.allowed_senses == {Sense.Minimize}


def test_direct_accepts_arbitrary_degree_binary_minimization_without_mutation() -> None:
    variables = [DecisionVariable.binary(i) for i in range(4)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=variables[0] * variables[1] * variables[2] * variables[3],
        constraints={},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.is_applicable
    assert report.input_membership.matching_clauses == [(0, "openjij-binary-hubo")]
    assert report.preconditions_checked
    assert report.precondition_violations == ()

    adapter = OMMXOpenJijSAAdapter(instance)
    assert adapter.ommx_instance.to_v2_bytes() == before
    assert instance.to_v2_bytes() == before


def test_direct_rejects_nonfinite_aggregated_interactions() -> None:
    x = DecisionVariable.binary(0)
    maximum = float.fromhex("0x1.fffffffffffffp+1023")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=maximum * x + maximum * x * x,
        constraints={},
        sense=Sense.Minimize,
    )

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.input_membership.is_member
    assert report.preconditions_checked
    assert not report.is_applicable
    [violation] = report.precondition_violations
    assert violation.condition == "openjij.interactions.coefficient_finite"
    assert violation.variable_ids == frozenset({0})


def test_direct_rejects_variable_id_outside_openjij_signed_range() -> None:
    accepted = _instance_with_variable(DecisionVariable.binary(2**63 - 1))
    assert OMMXOpenJijSAAdapter.check_applicability(accepted).is_applicable

    variable_id = 2**63
    instance = _instance_with_variable(DecisionVariable.binary(variable_id))

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.input_membership.is_member
    assert not report.is_applicable
    [violation] = report.precondition_violations
    assert violation.condition == "openjij.variable_id.signed_64_bit"
    assert violation.variable_ids == frozenset({variable_id})
    assert violation.limit == 2**63 - 1


@pytest.mark.parametrize(
    ("variable", "kind"),
    [
        (DecisionVariable.integer(0, lower=-2, upper=2), Kind.Integer),
        (DecisionVariable.continuous(0, lower=-2, upper=2), Kind.Continuous),
    ],
)
def test_direct_rejects_non_binary_variable_kind_without_mutation(
    variable: DecisionVariable,
    kind: Kind,
) -> None:
    instance = _instance_with_variable(variable)

    mismatches = _assert_direct_rejection_does_not_mutate(instance)
    [mismatch] = mismatches
    assert isinstance(mismatch, InstanceClassMismatch.VariableKindNotAllowed)
    assert mismatch.kind == kind
    assert mismatch.variable_ids == {0}
    assert mismatch.allowed_kinds == {Kind.Binary}


def test_direct_rejects_maximization_without_mutation() -> None:
    instance = _instance_with_variable(DecisionVariable.binary(0), sense=Sense.Maximize)

    mismatches = _assert_direct_rejection_does_not_mutate(instance)
    [mismatch] = mismatches
    assert isinstance(mismatch, InstanceClassMismatch.SenseNotAllowed)
    assert mismatch.sense == Sense.Maximize
    assert mismatch.allowed_senses == {Sense.Minimize}


def test_direct_rejects_regular_constraints_without_mutation() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x <= 0},
        sense=Sense.Minimize,
    )

    mismatches = _assert_direct_rejection_does_not_mutate(instance)
    [mismatch] = mismatches
    assert isinstance(
        mismatch,
        InstanceClassMismatch.RegularConstraintRelationNotAllowed,
    )
    assert mismatch.relation == Equality.LessThanOrEqualToZero
    assert mismatch.constraint_ids == {7}
    assert mismatch.allowed_relations == set()


def test_direct_rejects_all_special_constraint_families_without_mutation() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={20: OneHotConstraint(variables=[x, y])},
        sos1_constraints={30: Sos1Constraint(variables=[x, y])},
        sense=Sense.Minimize,
    )

    mismatches = _assert_direct_rejection_does_not_mutate(instance)
    by_type = {type(mismatch): mismatch for mismatch in mismatches}

    indicator = by_type[InstanceClassMismatch.IndicatorConstraintsNotAllowed]
    assert isinstance(indicator, InstanceClassMismatch.IndicatorConstraintsNotAllowed)
    assert indicator.constraint_ids == {10}
    one_hot = by_type[InstanceClassMismatch.OneHotConstraintsNotAllowed]
    assert isinstance(one_hot, InstanceClassMismatch.OneHotConstraintsNotAllowed)
    assert one_hot.constraint_ids == {20}
    sos1 = by_type[InstanceClassMismatch.Sos1ConstraintsNotAllowed]
    assert isinstance(sos1, InstanceClassMismatch.Sos1ConstraintsNotAllowed)
    assert sos1.constraint_ids == {30}


def test_explicit_preparation_lowers_all_special_constraint_families() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={20: OneHotConstraint(variables=[x, y])},
        sos1_constraints={30: Sos1Constraint(variables=[x, y])},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(instance, uniform_penalty_weight=3.0)
    assert report.is_successful
    prepared = OMMXOpenJijSAAdapter.prepare(instance, uniform_penalty_weight=3.0)
    operations = {step.operation for step in prepared.report.steps}
    assert {
        "indicator_lowering",
        "one_hot_lowering",
        "sos1_lowering",
        "finite_penalty",
    } <= operations
    prepared_input = prepared.input
    assert prepared_input.constraints == {}
    assert prepared_input.indicator_constraints == {}
    assert prepared_input.one_hot_constraints == {}
    assert prepared_input.sos1_constraints == {}
    assert set(instance.indicator_constraints) == {10}
    assert set(instance.one_hot_constraints) == {20}
    assert set(instance.sos1_constraints) == {30}


def test_special_constraint_preparation_requires_uniform_penalty() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={20: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(instance, penalty_weights={})
    assert not report.is_successful
    [violation] = report.source_check.precondition_violations
    assert violation.condition == "openjij.penalty.special_requires_uniform"
    assert violation.constraint_refs == frozenset({ConstraintRef("one_hot", 20)})


def test_special_constraint_requires_explicit_finite_penalty_selection() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={20: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    [violation] = report.source_check.precondition_violations
    assert violation.condition == "openjij.penalty.explicit_selection"
    assert violation.constraint_refs == frozenset({ConstraintRef("one_hot", 20)})
    assert "constraints remaining" in violation.description.lower()
    assert "regular constraints" not in violation.description.lower()


def test_integer_sos1_is_lowered_before_log_encoding() -> None:
    integer = DecisionVariable.integer(0, lower=-1, upper=1)
    binary = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[integer, binary],
        objective=integer + binary,
        constraints={},
        sos1_constraints={30: Sos1Constraint(variables=[integer, binary])},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(instance, uniform_penalty_weight=4.0)
    assert report.is_successful
    prepared = OMMXOpenJijSAAdapter.prepare(instance, uniform_penalty_weight=4.0)
    operations = [step.operation for step in prepared.report.steps]
    assert operations.index("sos1_lowering") < operations.index("integer_log_encoding")
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable
    assert set(instance.sos1_constraints) == {30}


def test_check_preparation_accepts_finite_integer_encoding() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0, lower=-3, upper=5))

    report = _check_preparation_without_mutation(instance)
    assert report.is_successful
    assert report.source_check.source_membership.is_member
    assert report.source_check.preconditions_checked
    assert report.source_check.precondition_violations == ()


def test_check_preparation_reports_unbounded_integer_encoding_precondition() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0))

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    assert report.source_check.preconditions_checked
    [violation] = report.source_check.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset({0})
    assert violation.constraint_refs == frozenset()
    assert "finite" in _violation_text(violation)


def test_check_preparation_reports_more_than_53_log_encoding_bits() -> None:
    instance = _instance_with_variable(
        DecisionVariable.integer(0, lower=0, upper=float(2**53))
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    assert report.source_check.preconditions_checked
    [violation] = report.source_check.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset({0})
    assert violation.actual == 54
    assert violation.limit == 53
    assert "53" in _violation_text(violation)


def test_check_preparation_reports_slack_range_outside_u64() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 2 * x - 1 <= 0},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(
        instance,
        uniform_penalty_weight=2.0,
        inequality_integer_slack_max_range=2**64,
    )
    assert not report.is_successful
    [violation] = report.source_check.precondition_violations
    assert violation.condition == "openjij.slack.range_unsigned_64_bit"
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert violation.limit == f"integer in [1, {2**64 - 1}]"


def test_check_preparation_reports_non_point_range_too_far_from_zero() -> None:
    max_exact_integer = float(2**53)
    upper = float(2**53 + 2)
    instance = _instance_with_variable(
        DecisionVariable.integer(0, lower=max_exact_integer, upper=upper)
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    assert report.source_check.preconditions_checked
    [violation] = report.source_check.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset({0})
    assert violation.actual == upper
    assert violation.limit == max_exact_integer
    assert "unit" in _violation_text(violation)


def test_check_preparation_requires_an_explicit_penalty_for_constraints() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    assert report.source_check.preconditions_checked
    [violation] = report.source_check.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset()
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert "penalty" in _violation_text(violation)

    accepted = _check_preparation_without_mutation(instance, uniform_penalty_weight=3.0)
    assert accepted.is_successful


def test_per_constraint_penalty_preserves_u64_constraint_id() -> None:
    constraint_id = 2**63
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={constraint_id: x == 0},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(
        instance, penalty_weights={constraint_id: 2.0}
    )
    assert report.is_successful
    prepared = OMMXOpenJijSAAdapter.prepare(
        instance, penalty_weights={constraint_id: 2.0}
    )
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable


def test_check_preparation_reports_penalty_materialization_overflow() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 2 * x == 0},
        sense=Sense.Minimize,
    )
    weight = float.fromhex("0x1.fffffffffffffp+1023")

    report = _check_preparation_without_mutation(
        instance, uniform_penalty_weight=weight
    )
    assert not report.is_successful
    [violation] = report.source_check.precondition_violations
    assert violation.condition == "openjij.preparation.materialization"
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})

    with pytest.raises(OpenJijPreparationError) as error:
        OMMXOpenJijSAAdapter.prepare(
            instance,
            uniform_penalty_weight=weight,
        )
    assert error.value.report.source_check.precondition_violations == (violation,)


def test_preparation_surfaces_proven_infeasibility() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x + 1 <= 0},
        sense=Sense.Minimize,
    )

    with pytest.raises(InfeasibleDetected):
        OMMXOpenJijSAAdapter.check_preparation(
            instance,
            uniform_penalty_weight=2.0,
        )
    with pytest.raises(InfeasibleDetected):
        OMMXOpenJijSAAdapter.prepare(
            instance,
            uniform_penalty_weight=2.0,
        )


def test_prepare_exact_maximization_and_integer_encoding() -> None:
    x = DecisionVariable.integer(5, lower=-2, upper=3)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x * x + x,
        constraints={},
        sense=Sense.Maximize,
    )
    before = instance.to_v2_bytes()

    prepared = OMMXOpenJijSAAdapter.prepare(instance)
    assert isinstance(prepared, OpenJijPreparation)
    assert isinstance(prepared.report, OpenJijPreparationReport)
    assert prepared.report.source_check.conditions_hold
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable
    assert final.input_membership.matching_clauses == [(0, "openjij-binary-hubo")]

    assert len(prepared.report.steps) >= 2
    assert all(step.operation for step in prepared.report.steps)
    assert "approximate_integer_slack" not in {
        step.operation for step in prepared.report.steps
    }
    assert "finite_penalty" not in {step.operation for step in prepared.report.steps}
    assert any(step.variable_ids == frozenset({5}) for step in prepared.report.steps)
    prepared_input = prepared.input
    assert prepared_input.sense == Sense.Minimize
    assert prepared_input.constraints == {}
    assert {variable.kind for variable in prepared_input.used_decision_variables} == {
        DecisionVariable.BINARY
    }
    assert instance.sense == Sense.Maximize
    assert instance.constraints == {}
    assert instance.to_v2_bytes() == before


def test_constrained_preparation_uses_exact_slack_by_default() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 3 * x - 2 <= 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    prepared = OMMXOpenJijSAAdapter.prepare(
        instance,
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=32,
    )
    report = prepared.report
    assert report.is_successful
    assert {step.operation for step in report.steps} >= {
        "exact_integer_slack",
        "finite_penalty",
    }
    assert "approximate_integer_slack" not in {step.operation for step in report.steps}
    assert prepared.input.constraints == {}
    assert set(instance.constraints) == {7}
    assert instance.to_v2_bytes() == before


def test_approximate_integer_slack_requires_explicit_selection() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 3 * x - 2 <= 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    report = _check_preparation_without_mutation(
        instance,
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=1,
    )
    assert not report.is_successful
    [violation] = report.source_check.precondition_violations
    assert violation.condition == "openjij.slack.approximation_explicit_selection"
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert violation.limit == "allow_approximate_integer_slack=True"

    with pytest.raises(OpenJijPreparationError) as error:
        OMMXOpenJijSAAdapter.prepare(
            instance,
            uniform_penalty_weight=4.0,
            inequality_integer_slack_max_range=1,
        )
    assert error.value.report == report
    assert instance.to_v2_bytes() == before


def test_approximate_integer_slack_can_be_selected_explicitly() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 3 * x - 2 <= 0},
        sense=Sense.Minimize,
    )

    prepared = OMMXOpenJijSAAdapter.prepare(
        instance,
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=1,
        allow_approximate_integer_slack=True,
    )
    assert prepared.report.is_successful
    assert {step.operation for step in prepared.report.steps} >= {
        "approximate_integer_slack",
        "finite_penalty",
    }


def test_preparation_rechecks_generated_variable_ids_for_openjij() -> None:
    x = DecisionVariable.binary(2**63 - 1)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 2 * x - 1 <= 0},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(
        instance,
        uniform_penalty_weight=2.0,
    )
    assert not report.is_successful
    final = report.input_applicability
    assert final is not None
    [violation] = final.precondition_violations
    assert violation.condition == "openjij.variable_id.signed_64_bit"
    assert min(violation.variable_ids) >= 2**63


def test_preparation_reports_trivially_satisfied_inequality_as_exact() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x - 2 <= 0},
        sense=Sense.Minimize,
    )

    prepared = OMMXOpenJijSAAdapter.prepare(instance)
    steps = prepared.report.steps
    [removal] = [
        step for step in steps if step.operation == "trivial_inequality_removal"
    ]
    assert removal.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert not [step for step in steps if step.operation == "finite_penalty"]
    assert set(instance.constraints) == {7}
