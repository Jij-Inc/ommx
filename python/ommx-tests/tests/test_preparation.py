from __future__ import annotations

import pytest

from ommx import (
    DecisionVariable,
    DegreeBound,
    ExactIntegerSlackError,
    FixedPenaltyPreparation,
    Function,
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
    assert instance.output_objective is not None
    assert not instance.output_objective.preserves_optimality


def test_prepare_preserves_output_objective_across_later_rewrites() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x,
        decision_variables=[x],
        constraints={},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=1,
    )

    instance.prepare(
        target,
        PreparationPolicy(sense=SensePreparation.as_minimization_problem()),
    )

    assert instance.sense == Sense.Minimize
    assert instance.objective.almost_equal(Function(-x))
    assert instance.output_objective is not None
    assert instance.output_objective.sense == Sense.Maximize
    assert instance.output_objective.function.almost_equal(Function(x))
    assert instance.output_objective.preserves_optimality
    assert instance.evaluate({0: 1}).sense == Sense.Maximize
    assert instance.evaluate({0: 1}).objective == pytest.approx(1.0)

    # Later active-formulation rewrites never negate or replace the established
    # output pair.
    assert instance.as_maximization_problem()
    assert instance.output_objective.sense == Sense.Maximize
    assert instance.output_objective.function.almost_equal(Function(x))
    assert instance.output_objective.preserves_optimality
    assert instance.as_minimization_problem()
    assert instance.output_objective.sense == Sense.Maximize
    assert instance.output_objective.function.almost_equal(Function(x))
    assert instance.output_objective.preserves_optimality


def test_prepare_noop_does_not_install_output_objective() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=1,
    )

    instance.prepare(target, PreparationPolicy())

    assert instance.output_objective is None


def test_setting_objective_rebases_output_semantics() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x,
        decision_variables=[x],
        constraints={},
    )
    target = unconstrained_class(
        allowed_variable_kinds={Kind.Binary},
        objective_degree=1,
    )
    instance.prepare(
        target,
        PreparationPolicy(sense=SensePreparation.as_minimization_problem()),
    )
    assert instance.output_objective is not None
    with pytest.raises(RuntimeError, match="output_objective"):
        instance.as_parametric_instance()

    instance.objective = 2 * x

    assert instance.output_objective is None
    assert instance.as_parametric_instance().objective.almost_equal(Function(2 * x))
    solution = instance.evaluate({0: 1})
    assert solution.sense == Sense.Minimize
    assert solution.objective == pytest.approx(2.0)


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
