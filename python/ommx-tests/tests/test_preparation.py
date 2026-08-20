from __future__ import annotations

import pytest

from ommx import (
    BinaryPowerPreparation,
    DecisionVariable,
    DegreeBound,
    ExactIntegerSlackError,
    FixedPenaltyPreparation,
    Instance,
    InstanceClass,
    InstanceClassClause,
    IntegerEncodingPreparation,
    IntegerSlackPreparation,
    Kind,
    ObjectivePreparation,
    OneHotConstraint,
    PreparationPolicy,
    PreparationTargetNotReachedError,
    Sense,
    SpecialConstraintKind,
    SpecialConstraintPreparation,
)


def unconstrained_class(
    *,
    allowed_variable_kinds: set[Kind],
    objective_degree: int,
    sense: Sense = Sense.Minimize,
) -> InstanceClass:
    return InstanceClass(
        [
            InstanceClassClause(
                label="target",
                allowed_variable_kinds=allowed_variable_kinds,
                objective_degree_bound=DegreeBound.at_most(objective_degree),
                allowed_senses={sense},
            )
        ]
    )


@pytest.mark.parametrize(
    ("source", "target"),
    [
        (Sense.Maximize, Sense.Minimize),
        (Sense.Minimize, Sense.Maximize),
    ],
)
def test_objective_preparation_converts_to_target_sense(
    source: Sense,
    target: Sense,
) -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=source,
        objective=x,
        decision_variables=[x],
        constraints={},
    )
    input_class = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=1,
        sense=target,
    )
    objective = ObjectivePreparation(target=target)
    policy = PreparationPolicy(objective=objective)

    assert objective.target == target
    assert policy.objective == objective
    assert instance.prepare(input_class, policy) is None
    assert instance.sense == target
    solution = instance.evaluate({0: 1})
    assert solution.sense == source
    assert solution.objective == 1


def test_qubo_hubo_policy_factories_are_fresh_complete_and_configurable() -> None:
    expected_special_constraints = (
        SpecialConstraintPreparation.lower_special_constraints(
            kinds={
                SpecialConstraintKind.Indicator,
                SpecialConstraintKind.OneHot,
                SpecialConstraintKind.Sos1,
            }
        )
    )
    expected_objective = ObjectivePreparation(target=Sense.Minimize)
    expected_encoding = IntegerEncodingPreparation.log_encode_all_used_integers()
    expected_penalty = FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
        weight=1.0
    )

    qubo = PreparationPolicy.for_qubo(inequality_integer_slack_max_range=17)
    hubo = PreparationPolicy.for_hubo(inequality_integer_slack_max_range=17)
    for policy in (qubo, hubo):
        assert policy.special_constraints == expected_special_constraints
        assert policy.objective == expected_objective
        assert policy.integer_slack == IntegerSlackPreparation(
            max_integer_range=17,
            slack_upper_bound=17,
        )
        assert policy.integer_encoding == expected_encoding
        assert policy.fixed_penalty == expected_penalty
    assert qubo.binary_power_reduction == BinaryPowerPreparation()
    assert hubo.binary_power_reduction is None

    keyed = PreparationPolicy.for_qubo(penalty_weights={3: 2.0})
    assert keyed.fixed_penalty == (
        FixedPenaltyPreparation.penalty_method_with_fixed_weights(weights={3: 2.0})
    )
    uniform = PreparationPolicy.for_hubo(uniform_penalty_weight=4.0)
    assert uniform.fixed_penalty == (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
    )

    first = PreparationPolicy.for_qubo()
    second = PreparationPolicy.for_qubo()
    assert first is not second
    first.fixed_penalty = None
    first.binary_power_reduction = None
    assert second.fixed_penalty == expected_penalty
    assert second.binary_power_reduction == BinaryPowerPreparation()

    with pytest.raises(ValueError, match="Both uniform_penalty_weight"):
        PreparationPolicy.for_qubo(
            uniform_penalty_weight=1.0,
            penalty_weights={3: 2.0},
        )


def test_prepare_mutates_in_place_and_establishes_target_membership() -> None:
    x = DecisionVariable.integer(0, lower=0, upper=3)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x + y,
        decision_variables=[x, y],
        constraints={10: x <= 2},
        one_hot_constraints={20: OneHotConstraint(variables=[y])},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=2,
    )

    policy = PreparationPolicy()
    policy.special_constraints = SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
    policy.objective = ObjectivePreparation(target=Sense.Minimize)
    policy.integer_slack = IntegerSlackPreparation(
        max_integer_range=1,
        slack_upper_bound=2,
    )
    policy.integer_encoding = IntegerEncodingPreparation.log_encode_all_used_integers()
    policy.fixed_penalty = (
        FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=2.0)
    )

    alias = instance
    assert not target.contains(instance)
    assert instance.prepare(target, policy) is None
    assert target.contains(alias)


def test_prepare_preserves_owner_signal_and_earlier_commits() -> None:
    x = DecisionVariable.integer(0, lower=0, upper=3)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x,
        decision_variables=[x],
        constraints={10: x <= 2},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Integer},
        objective_degree=1,
    )
    policy = PreparationPolicy(
        objective=ObjectivePreparation(target=Sense.Minimize),
        integer_slack=IntegerSlackPreparation(max_integer_range=1),
    )

    with pytest.raises(ExactIntegerSlackError):
        instance.prepare(target, policy)

    assert instance.sense == Sense.Minimize


def test_prepare_exposes_final_report_when_policy_is_exhausted() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={10: x == 1},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=1,
    )

    with pytest.raises(PreparationTargetNotReachedError) as error:
        instance.prepare(target, PreparationPolicy())

    assert not target.contains(instance)
    assert error.value.report == target.check_membership(instance)


@pytest.mark.parametrize("keyed", [False, True])
def test_fixed_penalty_weight_tolerance_reaches_the_owner_boundary(keyed: bool) -> None:
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=2,
    )

    def fixed_penalty(weight: float) -> FixedPenaltyPreparation:
        if keyed:
            return FixedPenaltyPreparation.penalty_method_with_fixed_weights(
                weights={10: weight}, atol=0.1
            )
        return FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
            weight=weight, atol=0.1
        )

    x = DecisionVariable.binary(0)
    accepted = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={10: x == 1},
    )
    accepted.prepare(
        target,
        PreparationPolicy(fixed_penalty=fixed_penalty(-0.1)),
    )
    assert target.contains(accepted)

    rejected = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={10: x == 1},
    )
    before = rejected.to_v2_bytes()
    with pytest.raises(ValueError, match="Fixed penalty weight"):
        rejected.prepare(
            target,
            PreparationPolicy(fixed_penalty=fixed_penalty(-0.100_001)),
        )
    assert rejected.to_v2_bytes() == before
