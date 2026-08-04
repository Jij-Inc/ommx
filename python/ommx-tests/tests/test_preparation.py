from __future__ import annotations

from collections.abc import Mapping
from types import MappingProxyType
from typing import cast

import pytest

from ommx import (
    DecisionVariable,
    DegreeBound,
    Equality,
    Instance,
    InstanceClass,
    InstanceClassClause,
    Kind,
    LogEncodingError,
    OneHotConstraint,
    PreparationPolicy,
    Samples,
    Sense,
    SpecialConstraintKind,
    State,
)


def binary_linear_class() -> InstanceClass:
    return InstanceClass(
        [
            InstanceClassClause(
                label="binary-linear",
                allowed_variable_kinds={Kind.Binary},
                objective_degree_bound=DegreeBound.at_most(1),
                allowed_senses={Sense.Minimize},
                regular_constraint_degree_bounds={
                    Equality.EqualToZero: DegreeBound.at_most(1),
                    Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
                },
            )
        ]
    )


def binary_unconstrained_class() -> InstanceClass:
    return InstanceClass(
        [
            InstanceClassClause(
                label="binary-unconstrained",
                allowed_variable_kinds={Kind.Binary},
                objective_degree_bound=DegreeBound.unbounded(),
                allowed_senses={Sense.Minimize},
            )
        ]
    )


def test_policy_snapshots_preparation_options() -> None:
    mutable_weights = {7: 2.0}
    penalty_weights: Mapping[int, float] = MappingProxyType(mutable_weights)
    policy = PreparationPolicy(
        input_class=binary_linear_class(),
        allowed_special_constraint_lowerings={
            SpecialConstraintKind.Indicator,
            SpecialConstraintKind.OneHot,
        },
        allow_integer_log_encoding=True,
        allow_sense_normalization=True,
        inequality_integer_slack_max_range=31,
        allow_approximate_integer_slack=True,
        penalty_weights=penalty_weights,
    )
    mutable_weights[7] = 9.0

    assert policy.allowed_special_constraint_lowerings == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
    }
    assert policy.allow_integer_log_encoding
    assert policy.allow_sense_normalization
    assert policy.inequality_integer_slack_max_range == 31
    assert policy.allow_approximate_integer_slack
    assert policy.uniform_penalty_weight is None
    assert policy.penalty_weights == {7: 2.0}

    returned_weights = policy.penalty_weights
    assert returned_weights is not None
    returned_weights[7] = 4.0
    assert policy.penalty_weights == {7: 2.0}


def test_policy_rejects_inconsistent_flat_options() -> None:
    input_class = binary_linear_class()

    with pytest.raises(ValueError, match="mutually exclusive"):
        PreparationPolicy(
            input_class=input_class,
            uniform_penalty_weight=2.0,
            penalty_weights={7: 2.0},
        )

    with pytest.raises(ValueError, match="requires inequality_integer_slack_max_range"):
        PreparationPolicy(
            input_class=input_class,
            allow_approximate_integer_slack=True,
        )

    with pytest.raises(ValueError, match="integer in"):
        PreparationPolicy(
            input_class=input_class,
            inequality_integer_slack_max_range=0,
        )


@pytest.mark.parametrize("weight", [0.0, -1.0, float("inf"), float("nan")])
def test_policy_rejects_non_positive_or_non_finite_penalty_weights(
    weight: float,
) -> None:
    input_class = binary_linear_class()

    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            input_class=input_class,
            uniform_penalty_weight=weight,
        )

    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            input_class=input_class,
            penalty_weights={7: weight},
        )


@pytest.mark.parametrize(
    "key",
    [True, False, -1, 2**64, 1.5, "7"],
)
def test_policy_rejects_invalid_per_constraint_penalty_keys(key: object) -> None:
    weights = cast(Mapping[int, float], {key: 2.0})

    with pytest.raises(ValueError, match="integer constraint IDs"):
        PreparationPolicy(
            input_class=binary_linear_class(),
            penalty_weights=weights,
        )


@pytest.mark.parametrize("weight", [True, False, "2.0", object()])
def test_policy_rejects_non_numeric_or_boolean_penalty_weights(
    weight: object,
) -> None:
    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            input_class=binary_linear_class(),
            uniform_penalty_weight=cast(float, weight),
        )

    weights = cast(Mapping[int, float], {7: weight})
    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            input_class=binary_linear_class(),
            penalty_weights=weights,
        )


def test_policy_rejects_non_mapping_penalty_weights() -> None:
    with pytest.raises(TypeError, match="Mapping"):
        PreparationPolicy(
            input_class=binary_linear_class(),
            penalty_weights=cast(Mapping[int, float], [(7, 2.0)]),
        )


@pytest.mark.parametrize(
    "max_range",
    [True, False, -1, 2**64, 1.5, "32"],
)
def test_policy_rejects_invalid_integer_slack_max_range(max_range: object) -> None:
    with pytest.raises(ValueError, match="integer in"):
        PreparationPolicy(
            input_class=binary_linear_class(),
            inequality_integer_slack_max_range=cast(int, max_range),
        )


def test_identity_preparation_returns_none_and_uses_instance_for_evaluation() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    result = instance.prepare(PreparationPolicy(input_class=binary_linear_class()))

    assert result is None

    state = State({0: 1.0})
    solution = instance.evaluate(state)
    assert solution.state.entries == {0: 1.0}
    assert solution.objective == pytest.approx(1.0)

    samples = Samples({3: {0: 1.0}, 8: {0: 0.0}})
    sample_set = instance.evaluate_samples(samples)
    assert sample_set.sample_ids() == {3, 8}
    assert sample_set.get(3).state.entries == {0: 1.0}
    assert sample_set.get(8).state.entries == {0: 0.0}


def test_lowered_special_constraint_is_evaluated_by_prepared_instance() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )
    result = instance.prepare(
        PreparationPolicy(
            input_class=binary_linear_class(),
            allowed_special_constraint_lowerings={SpecialConstraintKind.OneHot},
        )
    )

    assert result is None
    assert instance.one_hot_constraints == {}
    assert set(instance.removed_one_hot_constraints) == {7}
    assert len(instance.constraints) == 1

    solution = instance.evaluate(State({0: 1.0, 1: 0.0}))
    assert solution.feasible
    assert solution.objective == pytest.approx(1.0)

    sample_set = instance.evaluate_samples(Samples({12: {0: 0.0, 1: 1.0}}))
    assert sample_set.sample_ids() == {12}
    assert sample_set.get(12).feasible


def test_identity_does_not_interpret_unused_per_constraint_penalty_ids() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    policy = PreparationPolicy(
        input_class=binary_linear_class(),
        penalty_weights={999: 2.0},
    )

    result = instance.prepare(policy)

    assert result is None
    assert instance.to_v2_bytes() == before


def test_extra_unknown_per_constraint_penalty_id_fails_when_penalty_is_reached() -> (
    None
):
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={9: x == 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    policy = PreparationPolicy(
        input_class=binary_unconstrained_class(),
        penalty_weights={9: 2.0, 999: 3.0},
    )

    with pytest.raises(RuntimeError):
        instance.prepare(policy)

    assert instance.to_v2_bytes() == before


def test_per_constraint_penalty_tracks_existing_regular_constraint_id() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={9: x == 0},
        sense=Sense.Minimize,
    )
    result = instance.prepare(
        PreparationPolicy(
            input_class=binary_unconstrained_class(),
            penalty_weights={9: 2.0},
        )
    )

    assert result is None
    assert not instance.constraints
    assert set(instance.removed_constraints) == {9}
    assert instance.evaluate(State({0: 0.0})).objective == pytest.approx(0.0)


def test_penalty_failure_keeps_prior_in_place_special_constraint_lowering() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )
    policy = PreparationPolicy(
        input_class=binary_unconstrained_class(),
        allowed_special_constraint_lowerings={SpecialConstraintKind.OneHot},
        penalty_weights={},
    )
    with pytest.raises(RuntimeError):
        instance.prepare(policy)

    assert instance.one_hot_constraints == {}
    assert set(instance.removed_one_hot_constraints) == {7}
    assert len(instance.constraints) == 1


def test_operation_owned_error_is_preserved() -> None:
    x = DecisionVariable.integer(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    policy = PreparationPolicy(
        input_class=binary_linear_class(),
        allow_integer_log_encoding=True,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(LogEncodingError, match="bound must be finite"):
        instance.prepare(policy)

    assert instance.to_v2_bytes() == before
