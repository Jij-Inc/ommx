from __future__ import annotations

import pytest

from ommx import (
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
    OneHotConstraint,
    PreparationPolicy,
    PreparationTargetNotReachedError,
    Sense,
    SensePreparation,
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
    policy.sense = SensePreparation.as_minimization_problem()
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
        sense=SensePreparation.as_minimization_problem(),
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
