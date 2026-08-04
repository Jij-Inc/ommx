from __future__ import annotations

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
    PreparationPolicy,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx.adapter import AdapterNotApplicableError, AdapterPreconditionViolation
from ommx_openjij_adapter import OMMXOpenJijSAAdapter


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


def test_recommended_policy_uses_input_class_and_enables_common_operations() -> None:
    policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()

    assert isinstance(policy, PreparationPolicy)
    [policy_input_clause] = policy.input_class.clauses
    [input_clause] = OMMXOpenJijSAAdapter.INPUT_CLASS.clauses  # type: ignore[union-attr]
    assert policy_input_clause.label == input_clause.label
    assert policy.allowed_special_constraint_lowerings == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    }
    assert policy.allow_integer_log_encoding
    assert policy.allow_sense_normalization
    assert policy.inequality_integer_slack_max_range == 32
    assert policy.inequality_integer_slack_max_error is None
    assert policy.uniform_penalty_weight is None
    assert policy.penalty_weights is None


def test_recommended_policy_accepts_explicit_penalty_and_slack_values() -> None:
    uniform = OMMXOpenJijSAAdapter.recommended_preparation_policy(
        uniform_penalty_weight=3.0,
        inequality_integer_slack_max_range=8,
        inequality_integer_slack_max_error=0.25,
    )
    assert uniform.uniform_penalty_weight == 3.0
    assert uniform.penalty_weights is None
    assert uniform.inequality_integer_slack_max_range == 8
    assert uniform.inequality_integer_slack_max_error == 0.25

    weights = {7: 2.0}
    per_constraint = OMMXOpenJijSAAdapter.recommended_preparation_policy(
        penalty_weights=weights,
    )
    weights[7] = 4.0
    assert per_constraint.uniform_penalty_weight is None
    assert per_constraint.penalty_weights == {7: 2.0}


def test_recommended_policy_rejects_incompatible_penalty_values() -> None:
    with pytest.raises(ValueError, match="mutually exclusive"):
        OMMXOpenJijSAAdapter.recommended_preparation_policy(
            uniform_penalty_weight=2.0,
            penalty_weights={7: 2.0},
        )


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


def test_direct_precondition_violation_is_structured() -> None:
    instance = _instance_with_variable(DecisionVariable.binary(2**63))
    report = OMMXOpenJijSAAdapter.check_applicability(instance)

    assert report.input_membership.is_member
    [violation] = report.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
