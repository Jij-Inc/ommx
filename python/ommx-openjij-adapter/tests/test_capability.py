from __future__ import annotations

from typing import Any

import pytest

from ommx import (
    DecisionVariable,
    DegreeLimit,
    Equality,
    IndicatorConstraint,
    Instance,
    Kind,
    OneHotConstraint,
    PortableCapabilityMismatch,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import (
    AdapterCompatibilityError,
    AdapterCompatibilityReport,
    AdapterPreconditionViolation,
    ConstraintRef,
    InfeasibleDetected,
)
from ommx_openjij_adapter import (
    OMMXOpenJijSAAdapter,
    OpenJijPreparationReport,
    OpenJijPreparedModel,
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
) -> tuple[PortableCapabilityMismatch, ...]:
    before = instance.to_v2_bytes()

    report = OMMXOpenJijSAAdapter.check_compatibility(instance)
    assert not report.compatible
    assert not report.portable_report.compatible
    assert not report.preconditions_checked
    assert report.precondition_violations == ()
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterCompatibilityError) as error:
        OMMXOpenJijSAAdapter(instance)
    assert error.value.report.adapter == report.adapter
    assert not error.value.report.compatible
    assert instance.to_v2_bytes() == before

    [profile_report] = report.portable_report.profiles
    return tuple(profile_report.mismatches)


def _check_preparation_without_mutation(
    instance: Instance,
    **kwargs: Any,
) -> AdapterCompatibilityReport:
    before = instance.to_v2_bytes()
    report = OMMXOpenJijSAAdapter.check_preparation(instance, **kwargs)
    assert instance.to_v2_bytes() == before
    return report


def _violation_text(violation: AdapterPreconditionViolation) -> str:
    return f"{violation.condition} {violation.description}".lower()


def test_declares_native_binary_polynomial_profile() -> None:
    capabilities = OMMXOpenJijSAAdapter.CAPABILITIES
    assert capabilities is not None

    [profile] = capabilities.profiles
    assert profile.name == "openjij-binary-hubo"
    assert profile.variable_kinds == {Kind.Binary}
    assert profile.objective_degree == DegreeLimit.any()
    assert profile.regular_constraints == {}
    assert profile.indicator_constraints == {}
    assert not profile.supports_one_hot
    assert not profile.supports_sos1
    assert profile.senses == {Sense.Minimize}


def test_direct_accepts_arbitrary_degree_binary_minimization_without_mutation() -> None:
    variables = [DecisionVariable.binary(i) for i in range(4)]
    instance = Instance.from_components(
        decision_variables=variables,
        objective=variables[0] * variables[1] * variables[2] * variables[3],
        constraints={},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    report = OMMXOpenJijSAAdapter.check_compatibility(instance)
    assert report.compatible
    assert report.portable_report.matching_profiles == ["openjij-binary-hubo"]
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

    report = OMMXOpenJijSAAdapter.check_compatibility(instance)
    assert report.portable_report.compatible
    assert report.preconditions_checked
    assert not report.compatible
    [violation] = report.precondition_violations
    assert violation.condition == "openjij.interactions.coefficient_finite"
    assert violation.variable_ids == frozenset({0})


def test_direct_rejects_variable_id_outside_openjij_signed_range() -> None:
    accepted = _instance_with_variable(DecisionVariable.binary(2**63 - 1))
    assert OMMXOpenJijSAAdapter.check_compatibility(accepted).compatible

    variable_id = 2**63
    instance = _instance_with_variable(DecisionVariable.binary(variable_id))

    report = OMMXOpenJijSAAdapter.check_compatibility(instance)
    assert report.portable_report.compatible
    assert not report.compatible
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
    assert isinstance(mismatch, PortableCapabilityMismatch.UnsupportedVariableKind)
    assert mismatch.kind == kind
    assert mismatch.used_variable_ids == {0}
    assert mismatch.supported_kinds == {Kind.Binary}


def test_direct_rejects_maximization_without_mutation() -> None:
    instance = _instance_with_variable(DecisionVariable.binary(0), sense=Sense.Maximize)

    mismatches = _assert_direct_rejection_does_not_mutate(instance)
    [mismatch] = mismatches
    assert isinstance(mismatch, PortableCapabilityMismatch.UnsupportedSense)
    assert mismatch.sense == Sense.Maximize
    assert mismatch.supported_senses == {Sense.Minimize}


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
        PortableCapabilityMismatch.UnsupportedRegularConstraintRelation,
    )
    assert mismatch.relation == Equality.LessThanOrEqualToZero
    assert mismatch.constraint_ids == {7}
    assert mismatch.supported_relations == set()


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

    indicator = by_type[PortableCapabilityMismatch.UnsupportedIndicatorConstraints]
    assert isinstance(
        indicator, PortableCapabilityMismatch.UnsupportedIndicatorConstraints
    )
    assert indicator.constraint_ids == {10}
    one_hot = by_type[PortableCapabilityMismatch.UnsupportedOneHotConstraints]
    assert isinstance(one_hot, PortableCapabilityMismatch.UnsupportedOneHotConstraints)
    assert one_hot.constraint_ids == {20}
    sos1 = by_type[PortableCapabilityMismatch.UnsupportedSos1Constraints]
    assert isinstance(sos1, PortableCapabilityMismatch.UnsupportedSos1Constraints)
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
    assert report.compatible
    prepared = OMMXOpenJijSAAdapter.prepare(instance, uniform_penalty_weight=3.0)
    operations = {step.operation for step in prepared.report.steps}
    assert {
        "indicator_lowering",
        "one_hot_lowering",
        "sos1_lowering",
        "finite_penalty",
    } <= operations
    assert prepared.solver_instance.constraints == {}
    assert prepared.solver_instance.indicator_constraints == {}
    assert prepared.solver_instance.one_hot_constraints == {}
    assert prepared.solver_instance.sos1_constraints == {}
    assert set(prepared.evaluation_instance.indicator_constraints) == {10}
    assert set(prepared.evaluation_instance.one_hot_constraints) == {20}
    assert set(prepared.evaluation_instance.sos1_constraints) == {30}


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
    assert not report.compatible
    [violation] = report.precondition_violations
    assert violation.condition == "openjij.penalty.special_requires_uniform"
    assert violation.constraint_refs == frozenset({ConstraintRef("one_hot", 20)})


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
    assert report.compatible
    prepared = OMMXOpenJijSAAdapter.prepare(instance, uniform_penalty_weight=4.0)
    operations = [step.operation for step in prepared.report.steps]
    assert operations.index("sos1_lowering") < operations.index("integer_log_encoding")
    assert prepared.report.final_compatibility.compatible
    assert set(prepared.evaluation_instance.sos1_constraints) == {30}


def test_check_preparation_accepts_finite_integer_encoding() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0, lower=-3, upper=5))

    report = _check_preparation_without_mutation(instance)
    assert report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    assert report.precondition_violations == ()


def test_check_preparation_reports_unbounded_integer_encoding_precondition() -> None:
    instance = _instance_with_variable(DecisionVariable.integer(0))

    report = _check_preparation_without_mutation(instance)
    assert not report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    [violation] = report.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset({0})
    assert violation.constraint_refs == frozenset()
    assert "finite" in _violation_text(violation)


def test_check_preparation_reports_more_than_53_log_encoding_bits() -> None:
    instance = _instance_with_variable(
        DecisionVariable.integer(0, lower=0, upper=float(2**53))
    )

    report = _check_preparation_without_mutation(instance)
    assert not report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    [violation] = report.precondition_violations
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
    assert not report.compatible
    [violation] = report.precondition_violations
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
    assert not report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    [violation] = report.precondition_violations
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
    assert not report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    [violation] = report.precondition_violations
    assert isinstance(violation, AdapterPreconditionViolation)
    assert violation.variable_ids == frozenset()
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert "penalty" in _violation_text(violation)

    accepted = _check_preparation_without_mutation(instance, uniform_penalty_weight=3.0)
    assert accepted.compatible


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
    assert report.compatible
    prepared = OMMXOpenJijSAAdapter.prepare(
        instance, penalty_weights={constraint_id: 2.0}
    )
    assert prepared.report.final_compatibility.compatible


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
    assert not report.compatible
    [violation] = report.precondition_violations
    assert violation.condition == "openjij.preparation.materialization"
    assert violation.constraint_refs == frozenset({ConstraintRef("regular", 7)})

    with pytest.raises(AdapterCompatibilityError) as error:
        OMMXOpenJijSAAdapter.prepare(
            instance,
            uniform_penalty_weight=weight,
        )
    assert error.value.report.precondition_violations == (violation,)


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
    assert isinstance(prepared, OpenJijPreparedModel)
    assert isinstance(prepared.report, OpenJijPreparationReport)
    assert prepared.report.source_compatibility.compatible
    assert prepared.report.encoding_compatibility.compatible
    assert prepared.report.final_compatibility.compatible
    assert prepared.report.final_compatibility.portable_report.matching_profiles == [
        "openjij-binary-hubo"
    ]

    assert len(prepared.report.steps) >= 2
    assert all(step.operation for step in prepared.report.steps)
    assert all(step.semantics.name == "Exact" for step in prepared.report.steps)
    assert any(step.variable_ids == frozenset({5}) for step in prepared.report.steps)
    solver_instance = prepared.solver_instance
    assert solver_instance.sense == Sense.Minimize
    assert solver_instance.constraints == {}
    assert {variable.kind for variable in solver_instance.used_decision_variables} == {
        DecisionVariable.BINARY
    }
    evaluation_instance = prepared.evaluation_instance
    assert evaluation_instance.sense == Sense.Maximize
    assert evaluation_instance.constraints == {}
    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize(
    ("max_range", "expected_slack_semantics"),
    [(32, "Exact"), (1, "Approximate")],
)
def test_constrained_preparation_classifies_slack_and_finite_penalty_steps(
    max_range: int,
    expected_slack_semantics: str,
) -> None:
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
        inequality_integer_slack_max_range=max_range,
    )
    report = prepared.report
    assert report.source_compatibility.compatible
    assert report.encoding_compatibility.compatible
    assert report.final_compatibility.compatible
    assert all(step.operation for step in report.steps)

    constraint_ref = ConstraintRef("regular", 7)
    slack_steps = [
        step
        for step in report.steps
        if constraint_ref in step.constraint_refs
        and step.semantics.name == expected_slack_semantics
    ]
    assert len(slack_steps) == 1

    penalty_steps = [
        step
        for step in report.steps
        if constraint_ref in step.constraint_refs
        and step.semantics.name == "FinitePenalty"
    ]
    assert len(penalty_steps) == 1
    assert prepared.solver_instance.constraints == {}
    assert set(prepared.evaluation_instance.constraints) == {7}
    assert instance.to_v2_bytes() == before


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
    assert not report.compatible
    [violation] = report.precondition_violations
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

    prepared = OMMXOpenJijSAAdapter.prepare(
        instance,
        uniform_penalty_weight=4.0,
    )
    steps = prepared.report.steps
    [removal] = [
        step for step in steps if step.operation == "trivial_inequality_removal"
    ]
    assert removal.semantics.name == "Exact"
    assert removal.constraint_refs == frozenset({ConstraintRef("regular", 7)})
    assert not [step for step in steps if step.operation == "finite_penalty"]
    assert set(prepared.evaluation_instance.constraints) == {7}
