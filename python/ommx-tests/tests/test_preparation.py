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
        acceptable_instance_class=binary_linear_class(),
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
    target = binary_linear_class()

    with pytest.raises(ValueError, match="mutually exclusive"):
        PreparationPolicy(
            acceptable_instance_class=target,
            uniform_penalty_weight=2.0,
            penalty_weights={7: 2.0},
        )

    with pytest.raises(ValueError, match="requires inequality_integer_slack_max_range"):
        PreparationPolicy(
            acceptable_instance_class=target,
            allow_approximate_integer_slack=True,
        )

    with pytest.raises(ValueError, match="integer in"):
        PreparationPolicy(
            acceptable_instance_class=target,
            inequality_integer_slack_max_range=0,
        )


@pytest.mark.parametrize("weight", [0.0, -1.0, float("inf"), float("nan")])
def test_policy_rejects_non_positive_or_non_finite_penalty_weights(
    weight: float,
) -> None:
    target = binary_linear_class()

    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            acceptable_instance_class=target,
            uniform_penalty_weight=weight,
        )

    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            acceptable_instance_class=target,
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
            acceptable_instance_class=binary_linear_class(),
            penalty_weights=weights,
        )


@pytest.mark.parametrize("weight", [True, False, "2.0", object()])
def test_policy_rejects_non_numeric_or_boolean_penalty_weights(
    weight: object,
) -> None:
    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            acceptable_instance_class=binary_linear_class(),
            uniform_penalty_weight=cast(float, weight),
        )

    weights = cast(Mapping[int, float], {7: weight})
    with pytest.raises(ValueError, match="positive finite"):
        PreparationPolicy(
            acceptable_instance_class=binary_linear_class(),
            penalty_weights=weights,
        )


def test_policy_rejects_non_mapping_penalty_weights() -> None:
    with pytest.raises(TypeError, match="Mapping"):
        PreparationPolicy(
            acceptable_instance_class=binary_linear_class(),
            penalty_weights=cast(Mapping[int, float], [(7, 2.0)]),
        )


@pytest.mark.parametrize(
    "max_range",
    [True, False, -1, 2**64, 1.5, "32"],
)
def test_policy_rejects_invalid_integer_slack_max_range(max_range: object) -> None:
    with pytest.raises(ValueError, match="integer in"):
        PreparationPolicy(
            acceptable_instance_class=binary_linear_class(),
            inequality_integer_slack_max_range=cast(int, max_range),
        )


def test_identity_preparation_uses_input_for_evaluation() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    preparation = instance.prepare(
        PreparationPolicy(acceptable_instance_class=binary_linear_class())
    )

    assert preparation.source.to_v2_bytes() == instance.to_v2_bytes()
    assert preparation.input.to_v2_bytes() == instance.to_v2_bytes()

    state = State({0: 1.0})
    solution = preparation.input.evaluate(state)
    assert solution.state.entries == {0: 1.0}
    assert solution.objective == pytest.approx(1.0)

    samples = Samples({3: {0: 1.0}, 8: {0: 0.0}})
    sample_set = preparation.input.evaluate_samples(samples)
    assert sample_set.sample_ids() == {3, 8}
    assert sample_set.get(3).state.entries == {0: 1.0}
    assert sample_set.get(8).state.entries == {0: 0.0}


def test_lowered_special_constraint_is_evaluated_by_prepared_input() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )
    preparation = instance.prepare(
        PreparationPolicy(
            acceptable_instance_class=binary_linear_class(),
            allowed_special_constraint_lowerings={SpecialConstraintKind.OneHot},
        )
    )

    assert preparation.input.one_hot_constraints == {}
    assert set(preparation.input.removed_one_hot_constraints) == {7}
    assert len(preparation.input.constraints) == 1

    solution = preparation.input.evaluate(State({0: 1.0, 1: 0.0}))
    assert solution.feasible
    assert solution.objective == pytest.approx(1.0)

    sample_set = preparation.input.evaluate_samples(Samples({12: {0: 0.0, 1: 1.0}}))
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
        acceptable_instance_class=binary_linear_class(),
        penalty_weights={999: 2.0},
    )

    preparation = instance.prepare(policy)

    assert preparation.input.to_v2_bytes() == before
    assert instance.to_v2_bytes() == before


def test_extra_unknown_per_constraint_penalty_id_fails_when_penalty_is_reached() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={9: x == 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    policy = PreparationPolicy(
        acceptable_instance_class=binary_unconstrained_class(),
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
    preparation = instance.prepare(
        PreparationPolicy(
            acceptable_instance_class=binary_unconstrained_class(),
            penalty_weights={9: 2.0},
        )
    )

    assert not preparation.input.constraints
    assert set(preparation.input.removed_constraints) == {9}
    assert preparation.input.evaluate(State({0: 0.0})).objective == pytest.approx(0.0)


def test_generated_regular_constraint_requires_uniform_penalty() -> None:
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
        acceptable_instance_class=binary_unconstrained_class(),
        allowed_special_constraint_lowerings={SpecialConstraintKind.OneHot},
        penalty_weights={},
    )
    before = instance.to_v2_bytes()

    with pytest.raises(RuntimeError):
        instance.prepare(policy)

    assert instance.to_v2_bytes() == before


def test_operation_owned_error_is_preserved() -> None:
    x = DecisionVariable.integer(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    policy = PreparationPolicy(
        acceptable_instance_class=binary_linear_class(),
        allow_integer_log_encoding=True,
    )

    with pytest.raises(LogEncodingError, match="bound must be finite"):
        instance.prepare(policy)
