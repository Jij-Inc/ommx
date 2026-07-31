from __future__ import annotations

import copy
from dataclasses import asdict, replace
from typing import cast

import pytest
import ommx_openjij_adapter._preparation_pipeline as preparation_pipeline

from ommx import (
    DecisionVariable,
    DegreeBound,
    Equality,
    IndicatorConstraint,
    InfeasibleDetected,
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
    Preparation,
    PreparationFailure,
    PreparationTransform,
)
from ommx_openjij_adapter import (
    OMMXOpenJijSAAdapter,
    OpenJijPreparationPolicy,
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
    policy: OpenJijPreparationPolicy | None = None,
) -> OpenJijPreparationReport:
    before = instance.to_v2_bytes()
    report = OMMXOpenJijSAAdapter.check_preparation(instance, policy=policy)
    assert instance.to_v2_bytes() == before
    return report


def _violation_text(violation: AdapterPreconditionViolation) -> str:
    return f"{violation.condition} {violation.description}".lower()


def _failure_text(failure: PreparationFailure) -> str:
    return f"{failure.reason} {failure.description}".lower()


def test_preparation_policy_owns_intrinsic_invariants() -> None:
    with pytest.raises(ValueError, match="mutually exclusive"):
        OpenJijPreparationPolicy(
            uniform_penalty_weight=2.0,
            penalty_weights={},
        )

    for weight in (0.0, -1.0, float("nan"), float("inf")):
        with pytest.raises(ValueError, match="finite positive"):
            OpenJijPreparationPolicy(uniform_penalty_weight=weight)
        with pytest.raises(ValueError, match="finite positive"):
            OpenJijPreparationPolicy(penalty_weights={7: weight})

    with pytest.raises(TypeError, match="must be a bool"):
        OpenJijPreparationPolicy(allow_approximate_integer_slack=cast(bool, 1))

    for constraint_id in (cast(int, True), -1, 2**64, cast(int, "7")):
        with pytest.raises(ValueError, match="constraint IDs"):
            OpenJijPreparationPolicy(penalty_weights={constraint_id: 2.0})


def test_preparation_policy_snapshots_per_constraint_weights() -> None:
    weights = {7: 2.0}
    policy = OpenJijPreparationPolicy(penalty_weights=weights)

    weights[7] = 3.0

    assert policy.penalty_weights == {7: 2.0}
    with pytest.raises(TypeError):
        cast(dict[int, float], policy.penalty_weights)[7] = 4.0
    assert policy.penalty_weights is not None
    with pytest.raises(AttributeError, match="immutable"):
        setattr(policy.penalty_weights, "_values", {})
    with pytest.raises(AttributeError, match="immutable"):
        delattr(policy.penalty_weights, "_values")
    assert copy.deepcopy(policy) == policy
    assert asdict(policy)["penalty_weights"] == {7: 2.0}


def test_preparation_report_rejects_impossible_outcome_combinations() -> None:
    rejected = _check_preparation_without_mutation(
        _instance_with_variable(DecisionVariable.continuous(0))
    )
    with pytest.raises(ValueError, match="rejected preparation source"):
        replace(
            rejected,
            transforms=(PreparationTransform(name="invalid", description="invalid"),),
        )

    prepared = _check_preparation_without_mutation(
        _instance_with_variable(DecisionVariable.binary(0))
    )
    with pytest.raises(ValueError, match="requires either phase failures"):
        replace(prepared, input_applicability=None)
    with pytest.raises(ValueError, match="both phase failures"):
        replace(
            prepared,
            preparation_failures=(
                PreparationFailure(
                    name="invalid",
                    reason="invalid",
                    description="invalid",
                ),
            ),
        )


def test_preparation_report_has_four_terminal_states() -> None:
    source_rejected = _check_preparation_without_mutation(
        _instance_with_variable(DecisionVariable.continuous(0))
    )
    assert not source_rejected.source_check.conditions_hold
    assert source_rejected.transforms == ()
    assert source_rejected.preparation_failures == ()
    assert source_rejected.input_applicability is None

    phase_rejected = _check_preparation_without_mutation(
        _instance_with_variable(DecisionVariable.integer(0))
    )
    assert phase_rejected.source_check.conditions_hold
    assert phase_rejected.preparation_failures
    assert phase_rejected.input_applicability is None

    candidate = _instance_with_variable(DecisionVariable.binary(2**63))
    candidate_rejected = _check_preparation_without_mutation(candidate)
    assert candidate_rejected.source_check.conditions_hold
    assert candidate_rejected.preparation_failures == ()
    assert candidate_rejected.input_applicability is not None
    assert not candidate_rejected.input_applicability.is_applicable

    successful = _check_preparation_without_mutation(
        _instance_with_variable(DecisionVariable.binary(0))
    )
    assert successful.source_check.conditions_hold
    assert successful.preparation_failures == ()
    assert successful.input_applicability is not None
    assert successful.input_applicability.is_applicable

    for instance, expected_report in (
        (_instance_with_variable(DecisionVariable.continuous(0)), source_rejected),
        (candidate, candidate_rejected),
    ):
        with pytest.raises(OpenJijPreparationError) as error:
            OMMXOpenJijSAAdapter.prepare(instance)
        assert error.value.report == expected_report


def test_penalty_policy_conditions_belong_to_the_penalty_phase() -> None:
    x = DecisionVariable.binary(0)
    unconstrained = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    unused_policy = OpenJijPreparationPolicy(uniform_penalty_weight=2.0)

    identity_report = _check_preparation_without_mutation(
        unconstrained,
        unused_policy,
    )

    assert identity_report.source_check.conditions_hold
    assert identity_report.is_successful
    assert identity_report.transforms == ()
    assert identity_report.preparation_failures == ()
    assert identity_report.policy is unused_policy

    constrained = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )
    incomplete_policy = OpenJijPreparationPolicy(penalty_weights={})

    incomplete_report = _check_preparation_without_mutation(
        constrained,
        incomplete_policy,
    )

    assert incomplete_report.source_check.conditions_hold
    [incomplete] = incomplete_report.preparation_failures
    assert incomplete.reason == "openjij.penalty.weight_coverage"
    assert incomplete.constraint_refs == frozenset({ConstraintRef("regular", 7)})


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

    policy = OpenJijPreparationPolicy(uniform_penalty_weight=3.0)
    report = _check_preparation_without_mutation(instance, policy)
    assert report.is_successful
    assert report.policy is policy
    prepared = OMMXOpenJijSAAdapter.prepare(instance, policy=policy)
    assert prepared.report.policy is policy
    operations = {step.name for step in prepared.report.transforms}
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

    policy = OpenJijPreparationPolicy(penalty_weights={})
    report = _check_preparation_without_mutation(instance, policy)
    assert not report.is_successful
    assert report.policy is policy
    assert report.source_check.conditions_hold
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.penalty.special_requires_uniform"
    assert failure.constraint_refs == frozenset({ConstraintRef("one_hot", 20)})


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
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.penalty.explicit_selection"
    assert failure.constraint_refs == frozenset({ConstraintRef("one_hot", 20)})
    assert "constraints remaining" in failure.description.lower()
    assert "regular constraints" not in failure.description.lower()


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

    policy = OpenJijPreparationPolicy(uniform_penalty_weight=4.0)
    report = _check_preparation_without_mutation(instance, policy)
    assert report.is_successful
    prepared = OMMXOpenJijSAAdapter.prepare(instance, policy=policy)
    operations = [step.name for step in prepared.report.transforms]
    assert operations.index("sos1_lowering") < operations.index("integer_log_encoding")
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable
    assert set(instance.sos1_constraints) == {30}


def test_check_preparation_accepts_finite_integer_encoding() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0, lower=-3, upper=5))

    report = _check_preparation_without_mutation(instance)
    assert report.is_successful
    assert report.source_check.source_membership.is_member


def test_source_integer_encoding_phase_reports_unbounded_integer() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0))

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    [failure] = report.preparation_failures
    assert isinstance(failure, PreparationFailure)
    assert failure.name == "integer_log_encoding"
    assert failure.reason == "openjij.log_encoding.bound_finite"
    assert failure.variable_ids == frozenset({0})
    assert failure.constraint_refs == frozenset()
    assert failure.expected == "finite integer range"
    assert "finite" in _failure_text(failure)


def test_source_integer_encoding_does_not_classify_id_exhaustion_as_unavailable() -> (
    None
):
    variable_id = 2**64 - 1
    instance = _instance_with_variable(
        DecisionVariable.integer(variable_id, lower=0, upper=3)
    )

    report = _check_preparation_without_mutation(instance)

    assert not report.is_successful
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.preparation.materialization"
    assert failure.variable_ids == frozenset({variable_id})
    assert "available decision variable id" in failure.description.lower()


def test_source_integer_encoding_phase_reports_more_than_53_bits() -> None:
    instance = _instance_with_variable(
        DecisionVariable.integer(0, lower=0, upper=float(2**53))
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    [failure] = report.preparation_failures
    assert failure.name == "integer_log_encoding"
    assert failure.reason == "openjij.log_encoding.max_bits"
    assert failure.variable_ids == frozenset({0})
    assert failure.observed == 54
    assert failure.expected == 53
    assert "too large" in _failure_text(failure)


def test_preparation_policy_rejects_slack_range_outside_u64() -> None:
    for slack_range in (0, 2**64, cast(int, True)):
        with pytest.raises(ValueError, match="integer in"):
            OpenJijPreparationPolicy(inequality_integer_slack_max_range=slack_range)


def test_source_integer_encoding_phase_reports_inexact_f64_range() -> None:
    max_exact_integer = float(2**53)
    upper = float(2**53 + 2)
    instance = _instance_with_variable(
        DecisionVariable.integer(0, lower=max_exact_integer, upper=upper)
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.is_successful
    assert report.source_check.source_membership.is_member
    [failure] = report.preparation_failures
    assert failure.name == "integer_log_encoding"
    assert failure.reason == "openjij.log_encoding.exact_integer_range"
    assert failure.variable_ids == frozenset({0})
    assert failure.observed == upper
    assert failure.expected == max_exact_integer
    assert "unit" in _failure_text(failure)


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
    [failure] = report.preparation_failures
    assert isinstance(failure, PreparationFailure)
    assert failure.variable_ids == frozenset()
    assert failure.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert "penalty" in _failure_text(failure)

    accepted = _check_preparation_without_mutation(
        instance,
        OpenJijPreparationPolicy(uniform_penalty_weight=3.0),
    )
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

    policy = OpenJijPreparationPolicy(penalty_weights={constraint_id: 2.0})
    report = _check_preparation_without_mutation(instance, policy)
    assert report.is_successful
    prepared = OMMXOpenJijSAAdapter.prepare(instance, policy=policy)
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable


def test_check_preparation_reports_penalty_materialization_overflow() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 2 * x == 0},
        sense=Sense.Maximize,
    )
    weight = float.fromhex("0x1.fffffffffffffp+1023")
    policy = OpenJijPreparationPolicy(uniform_penalty_weight=weight)

    report = _check_preparation_without_mutation(instance, policy)
    assert not report.is_successful
    assert report.policy is policy
    assert [step.name for step in report.transforms] == ["sense_reversal"]
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.preparation.materialization"
    assert failure.constraint_refs == frozenset({ConstraintRef("regular", 7)})

    with pytest.raises(OpenJijPreparationError) as error:
        OMMXOpenJijSAAdapter.prepare(
            instance,
            policy=policy,
        )
    assert error.value.report.preparation_failures == (failure,)
    assert error.value.report.transforms == report.transforms
    assert error.value.report.policy is policy


def test_preparation_surfaces_proven_infeasibility() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x + 1 <= 0},
        sense=Sense.Minimize,
    )
    policy = OpenJijPreparationPolicy(uniform_penalty_weight=2.0)
    before = instance.to_v2_bytes()

    with pytest.raises(InfeasibleDetected):
        OMMXOpenJijSAAdapter.check_preparation(
            instance,
            policy=policy,
        )
    with pytest.raises(InfeasibleDetected):
        OMMXOpenJijSAAdapter.prepare(
            instance,
            policy=policy,
        )
    assert instance.to_v2_bytes() == before


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
    assert isinstance(prepared, Preparation)
    assert isinstance(prepared.report, OpenJijPreparationReport)
    assert prepared.report.policy == OpenJijPreparationPolicy()
    assert prepared.report.source_check.conditions_hold
    final = prepared.report.input_applicability
    assert final is not None and final.is_applicable
    assert final.input_membership.matching_clauses == [(0, "openjij-binary-hubo")]

    assert len(prepared.report.transforms) >= 2
    assert all(step.name for step in prepared.report.transforms)
    assert "approximate_integer_slack" not in {
        step.name for step in prepared.report.transforms
    }
    assert "finite_penalty" not in {step.name for step in prepared.report.transforms}
    assert any(
        step.variable_ids == frozenset({5}) for step in prepared.report.transforms
    )
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

    policy = OpenJijPreparationPolicy(
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=32,
    )
    prepared = OMMXOpenJijSAAdapter.prepare(instance, policy=policy)
    report = prepared.report
    assert report.is_successful
    assert report.policy is policy
    assert {step.name for step in report.transforms} >= {
        "exact_integer_slack",
        "finite_penalty",
    }
    assert "approximate_integer_slack" not in {step.name for step in report.transforms}
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

    policy = OpenJijPreparationPolicy(
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=1,
    )
    report = _check_preparation_without_mutation(instance, policy)
    assert not report.is_successful
    assert report.policy is policy
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.slack.approximation_explicit_selection"
    assert failure.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert failure.expected == "allow_approximate_integer_slack=True"

    with pytest.raises(OpenJijPreparationError) as error:
        OMMXOpenJijSAAdapter.prepare(
            instance,
            policy=policy,
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

    policy = OpenJijPreparationPolicy(
        uniform_penalty_weight=4.0,
        inequality_integer_slack_max_range=1,
        allow_approximate_integer_slack=True,
    )
    prepared = OMMXOpenJijSAAdapter.prepare(instance, policy=policy)
    assert prepared.report.is_successful
    assert prepared.report.policy is policy
    assert {step.name for step in prepared.report.transforms} >= {
        "approximate_integer_slack",
        "finite_penalty",
    }


def test_approximate_slack_does_not_recover_exact_materialization_failure() -> None:
    variable_id = 2**64 - 1
    x = DecisionVariable.binary(variable_id)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: 2 * x - 1 <= 0},
        sense=Sense.Minimize,
    )

    report = _check_preparation_without_mutation(
        instance,
        OpenJijPreparationPolicy(
            uniform_penalty_weight=4.0,
            allow_approximate_integer_slack=True,
        ),
    )

    assert not report.is_successful
    [failure] = report.preparation_failures
    assert failure.reason == "openjij.preparation.materialization"
    assert failure.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert "exact integer slack" in failure.description.lower()
    assert "available decision variable id" in failure.description.lower()
    assert "approximate_integer_slack" not in {step.name for step in report.transforms}


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
        OpenJijPreparationPolicy(uniform_penalty_weight=2.0),
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
    transforms = prepared.report.transforms
    [removal] = [
        step for step in transforms if step.name == "trivial_inequality_removal"
    ]
    assert removal.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert not [step for step in transforms if step.name == "finite_penalty"]
    assert set(instance.constraints) == {7}


def test_penalty_coverage_uses_constraints_remaining_after_slack() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={
            7: x - 2 <= 0,
            8: x == 0,
        },
        sense=Sense.Minimize,
    )

    prepared = OMMXOpenJijSAAdapter.prepare(
        instance,
        policy=OpenJijPreparationPolicy(penalty_weights={8: 2.0}),
    )

    [removal] = [
        step
        for step in prepared.report.transforms
        if step.name == "trivial_inequality_removal"
    ]
    [penalty] = [
        step for step in prepared.report.transforms if step.name == "finite_penalty"
    ]
    assert removal.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert penalty.constraint_refs == frozenset({ConstraintRef("regular", 8)})


def test_known_weight_for_trivially_removed_constraint_is_optional() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x - 2 <= 0},
        sense=Sense.Minimize,
    )

    prepared = OMMXOpenJijSAAdapter.prepare(
        instance,
        policy=OpenJijPreparationPolicy(penalty_weights={}),
    )
    assert prepared.report.is_successful
    assert not [
        step for step in prepared.report.transforms if step.name == "finite_penalty"
    ]


def test_unexpected_phase_exception_is_not_a_preparation_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Maximize,
    )

    def broken_phase(_state: object, **_kwargs: object) -> object:
        raise AssertionError("phase invariant sentinel")

    monkeypatch.setattr(
        preparation_pipeline,
        "lower_special_constraints",
        broken_phase,
    )
    with pytest.raises(AssertionError, match="phase invariant sentinel"):
        OMMXOpenJijSAAdapter.check_preparation(instance)
