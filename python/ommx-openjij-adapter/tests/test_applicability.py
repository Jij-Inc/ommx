from __future__ import annotations

import pytest

from ommx import (
    DecisionVariable,
    DegreeBound,
    Equality,
    FixedPenaltyPreparation,
    IndicatorConstraint,
    Instance,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    PreparationTargetNotReachedError,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import AdapterNotApplicableError
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
    assert not report.is_member
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterNotApplicableError) as error:
        OMMXOpenJijSAAdapter(instance)
    assert error.value.report == report
    assert instance.to_v2_bytes() == before

    [clause_report] = report.clause_reports
    return tuple(clause_report.mismatches)


def test_declares_binary_polynomial_input_class() -> None:
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS

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
    assert report.is_member
    assert report.matching_clauses == [(0, "openjij-binary-hubo")]

    adapter = OMMXOpenJijSAAdapter(instance)
    assert adapter.ommx_instance.to_v2_bytes() == before
    assert instance.to_v2_bytes() == before


def test_sampler_input_rejects_nonfinite_aggregated_interactions() -> None:
    x = DecisionVariable.binary(0)
    maximum = float.fromhex("0x1.fffffffffffffp+1023")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=maximum * x + maximum * x * x,
        constraints={},
        sense=Sense.Minimize,
    )

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.is_member

    adapter = OMMXOpenJijSAAdapter(instance)
    with pytest.raises(ValueError, match="non-finite interaction coefficients"):
        _ = adapter.sampler_input
    assert not adapter._sampler_input_prepared
    assert adapter._hubo == {}
    assert adapter._qubo == {}


def test_sampler_input_rejects_variable_id_outside_openjij_signed_range() -> None:
    accepted = _instance_with_variable(DecisionVariable.binary(2**63 - 1))
    assert OMMXOpenJijSAAdapter.check_applicability(accepted).is_member
    _ = OMMXOpenJijSAAdapter(accepted).sampler_input

    variable_id = 2**63
    instance = _instance_with_variable(DecisionVariable.binary(variable_id))

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.is_member

    adapter = OMMXOpenJijSAAdapter(instance)
    with pytest.raises(ValueError, match="signed 64-bit integer"):
        _ = adapter.sampler_input
    assert not adapter._sampler_input_prepared
    assert adapter._hubo == {}
    assert adapter._qubo == {}


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
    mismatches = _assert_direct_rejection_does_not_mutate(
        _instance_with_variable(variable)
    )
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


def test_recommended_policy_keeps_fixed_penalty_caller_owned() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS

    policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    assert policy.fixed_penalty is None
    with pytest.raises(PreparationTargetNotReachedError):
        instance.prepare(input_class, policy)

    policy.fixed_penalty = (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=2.0)
    )
    fresh = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    assert fresh.fixed_penalty is None


def test_recommended_preparation_reaches_the_openjij_input_class() -> None:
    integer = DecisionVariable.integer(0, lower=0, upper=100)
    indicator = DecisionVariable.binary(1)
    x = DecisionVariable.binary(2)
    y = DecisionVariable.binary(3)
    instance = Instance.from_components(
        decision_variables=[integer, indicator, x, y],
        objective=integer + indicator + x + y,
        constraints={40: integer <= 50},
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=indicator,
                function=x - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={20: OneHotConstraint(variables=[indicator, x])},
        sos1_constraints={30: Sos1Constraint(variables=[x, y])},
        sense=Sense.Maximize,
    )
    alias = instance
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
    assert not input_class.contains(instance)

    policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
    policy.fixed_penalty = (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=2.0)
    )

    assert instance.prepare(input_class, policy) is None
    assert alias is instance
    assert input_class.contains(alias)
    assert OMMXOpenJijSAAdapter.check_applicability(alias).is_member


def test_sampler_input_validation_still_follows_preparation() -> None:
    x = DecisionVariable.binary(2**63)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Maximize,
    )
    input_class = OMMXOpenJijSAAdapter.INPUT_CLASS

    instance.prepare(
        input_class,
        OMMXOpenJijSAAdapter.recommended_preparation_policy(),
    )
    assert input_class.contains(instance)

    report = OMMXOpenJijSAAdapter.check_applicability(instance)
    assert report.is_member

    adapter = OMMXOpenJijSAAdapter(instance)
    with pytest.raises(ValueError, match="signed 64-bit integer"):
        _ = adapter.sampler_input
