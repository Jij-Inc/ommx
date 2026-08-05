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
    penalty_method_with_weights: Mapping[int, float] = MappingProxyType(mutable_weights)
    policy = PreparationPolicy(
        lower_special_constraints={
            SpecialConstraintKind.Indicator,
            SpecialConstraintKind.OneHot,
        },
        log_encode_used_integers=1e-4,
        as_minimization_problem=True,
        convert_inequality_to_equality_with_integer_slack=(32, 2e-4),
        add_integer_slack_to_inequality=32,
        penalty_method_with_weights=penalty_method_with_weights,
    )
    mutable_weights[7] = 9.0

    assert policy.lower_special_constraints == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
    }
    assert policy.log_encode_used_integers == 1e-4
    assert policy.as_minimization_problem
    assert policy.convert_inequality_to_equality_with_integer_slack == (32, 2e-4)
    assert policy.add_integer_slack_to_inequality == 32
    assert policy.uniform_penalty_method_with_weight is None
    assert policy.penalty_method_with_weights == {7: 2.0}

    returned_weights = policy.penalty_method_with_weights
    assert returned_weights is not None
    returned_weights[7] = 4.0
    assert policy.penalty_method_with_weights == {7: 2.0}


def test_policy_stores_tolerance_with_each_owner_operation() -> None:
    policy = PreparationPolicy(
        log_encode_used_integers=1e-4,
        convert_inequality_to_equality_with_integer_slack=(31, 2e-4),
    )

    assert policy.log_encode_used_integers == 1e-4
    assert policy.convert_inequality_to_equality_with_integer_slack == (31, 2e-4)


def test_policy_is_independent_of_the_target_input_class() -> None:
    policy = PreparationPolicy()
    assert not hasattr(policy, "input_class")

    x = DecisionVariable.binary(0)
    first = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    second = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    assert first.prepare(binary_linear_class(), policy) is None
    assert second.prepare(binary_unconstrained_class(), policy) is None


def test_policy_stores_alternative_penalty_operation_arguments_verbatim() -> None:
    policy = PreparationPolicy(
        uniform_penalty_method_with_weight=2.0,
        penalty_method_with_weights={7: 2.0},
    )

    assert policy.uniform_penalty_method_with_weight == 2.0
    assert policy.penalty_method_with_weights == {7: 2.0}


def test_policy_rejects_invalid_owner_atol_arguments() -> None:
    with pytest.raises(ValueError, match="ATol must be positive"):
        PreparationPolicy(log_encode_used_integers=0.0)

    with pytest.raises(ValueError, match="ATol must be positive"):
        PreparationPolicy(convert_inequality_to_equality_with_integer_slack=(32, 0.0))


@pytest.mark.parametrize("weight", [0.0, -1.0, float("inf"), float("nan")])
def test_uniform_penalty_owner_rejects_non_positive_or_non_finite_weight(
    weight: float,
) -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    policy = PreparationPolicy(uniform_penalty_method_with_weight=weight)

    with pytest.raises(RuntimeError, match="positive and finite"):
        instance.prepare(binary_unconstrained_class(), policy)

    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("weight", [0.0, -1.0, float("inf"), float("nan")])
def test_per_constraint_penalty_owner_rejects_invalid_weight(weight: float) -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    policy = PreparationPolicy(penalty_method_with_weights={7: weight})

    with pytest.raises(RuntimeError, match="positive and finite"):
        instance.prepare(binary_unconstrained_class(), policy)

    assert instance.to_v2_bytes() == before


@pytest.mark.parametrize(
    "key",
    [True, False, -1, 2**64, 1.5, "7"],
)
def test_policy_rejects_invalid_per_constraint_penalty_keys(key: object) -> None:
    weights = cast(Mapping[int, float], {key: 2.0})

    with pytest.raises(ValueError, match="integer constraint IDs"):
        PreparationPolicy(
            penalty_method_with_weights=weights,
        )


def test_policy_rejects_non_mapping_penalty_method_arguments() -> None:
    with pytest.raises(TypeError, match="Mapping"):
        PreparationPolicy(
            penalty_method_with_weights=cast(Mapping[int, float], [(7, 2.0)]),
        )


def test_log_encode_used_integers_is_a_direct_instance_operation() -> None:
    x = DecisionVariable.integer(0, lower=0, upper=3, name="x")
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    instance.log_encode_used_integers(atol=1e-6)

    assert instance.dependent_decision_variable_ids() == {0}
    encoded = [
        variable
        for variable in instance.decision_variables
        if variable.name == "ommx.log_encode"
    ]
    assert len(encoded) == 2
    assert {variable.kind for variable in encoded} == {Kind.Binary}


def test_uniform_penalty_with_weight_is_a_direct_instance_operation() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )

    instance.uniform_penalty_method_with_weight(2.0)

    assert not instance.constraints
    assert set(instance.removed_constraints) == {7}
    assert instance.evaluate(State({0: 1.0})).objective == pytest.approx(3.0)


def test_penalty_with_weights_is_a_direct_instance_operation() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=0.0,
        constraints={7: x == 0, 8: y == 0},
        sense=Sense.Minimize,
    )

    instance.penalty_method_with_weights({7: 2.0, 8: 5.0})

    assert not instance.constraints
    assert set(instance.removed_constraints) == {7, 8}
    assert instance.evaluate(State({0: 1.0, 1: 0.0})).objective == pytest.approx(2.0)
    assert instance.evaluate(State({0: 0.0, 1: 1.0})).objective == pytest.approx(5.0)


def test_direct_fixed_penalty_failure_is_atomic() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x == 0},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(RuntimeError, match="positive and finite"):
        instance.uniform_penalty_method_with_weight(0.0)

    assert instance.to_v2_bytes() == before


def test_identity_preparation_returns_none_and_uses_instance_for_evaluation() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )
    result = instance.prepare(binary_linear_class(), PreparationPolicy())

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
        binary_linear_class(),
        PreparationPolicy(
            lower_special_constraints={SpecialConstraintKind.OneHot},
        ),
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
        penalty_method_with_weights={999: 2.0},
    )

    result = instance.prepare(binary_linear_class(), policy)

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
        penalty_method_with_weights={9: 2.0, 999: 3.0},
    )

    with pytest.raises(RuntimeError):
        instance.prepare(binary_unconstrained_class(), policy)

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
        binary_unconstrained_class(),
        PreparationPolicy(
            penalty_method_with_weights={9: 2.0},
        ),
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
        lower_special_constraints={SpecialConstraintKind.OneHot},
        penalty_method_with_weights={},
    )
    with pytest.raises(RuntimeError):
        instance.prepare(binary_unconstrained_class(), policy)

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
        log_encode_used_integers=1e-6,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(LogEncodingError, match="bound must be finite"):
        instance.prepare(binary_linear_class(), policy)

    assert instance.to_v2_bytes() == before
