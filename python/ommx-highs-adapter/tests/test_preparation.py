import pytest

from ommx import (
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    PreparationError,
    PreparationFailure,
    PreparationPolicy,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx_highs_adapter import OMMXHighsAdapter


def _indicator_instance() -> Instance:
    x = DecisionVariable.continuous(0, lower=0, upper=2)
    trigger = DecisionVariable.binary(1)
    return Instance.from_components(
        decision_variables=[x, trigger],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=trigger,
                function=x - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
    )


def _one_hot_instance() -> Instance:
    x = [DecisionVariable.binary(i) for i in range(2)]
    return Instance.from_components(
        decision_variables=x,
        objective=x[0] + 2 * x[1],
        constraints={},
        sense=Sense.Minimize,
        one_hot_constraints={20: OneHotConstraint(variables=x)},
    )


def _sos1_instance() -> Instance:
    x = [
        DecisionVariable.continuous(0, lower=0, upper=2),
        DecisionVariable.continuous(1, lower=0, upper=3),
    ]
    return Instance.from_components(
        decision_variables=x,
        objective=-x[0] - 2 * x[1],
        constraints={},
        sense=Sense.Minimize,
        sos1_constraints={30: Sos1Constraint(variables=x)},
    )


def test_recommended_policy_targets_highs_input_class() -> None:
    policy = OMMXHighsAdapter.recommended_preparation_policy()

    assert isinstance(policy, PreparationPolicy)
    [target_clause] = policy.acceptable_instance_class.clauses
    assert target_clause.label == "highs-linear-mip"
    assert policy.allowed_special_constraint_lowerings == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    }


@pytest.mark.parametrize(
    ("make_source", "kind", "transform_name"),
    [
        (
            _indicator_instance,
            SpecialConstraintKind.Indicator,
            "lower_indicator_constraints",
        ),
        (
            _one_hot_instance,
            SpecialConstraintKind.OneHot,
            "lower_one_hot_constraints",
        ),
        (_sos1_instance, SpecialConstraintKind.Sos1, "lower_sos1_constraints"),
    ],
)
def test_instance_prepare_automatically_lowers_special_constraint(
    make_source, kind, transform_name
) -> None:
    source = make_source()
    before = source.to_v2_bytes()

    preparation = source.prepare(OMMXHighsAdapter.recommended_preparation_policy())

    assert preparation.source.to_v2_bytes() == before
    assert source.to_v2_bytes() == before
    assert kind not in preparation.input.active_special_constraint_kinds
    assert OMMXHighsAdapter.check_applicability(preparation.input).is_applicable
    assert [transform.name for transform in preparation.transforms] == [transform_name]


def test_sos1_preparation_decodes_input_state_for_source_evaluation() -> None:
    source = _sos1_instance()
    preparation = source.prepare(OMMXHighsAdapter.recommended_preparation_policy())

    input_solution = OMMXHighsAdapter.solve(preparation.input)
    source_state = preparation.decode(input_solution.state)
    source_solution = preparation.source.evaluate(
        source_state,
        atol=preparation.policy.atol,
    )

    assert source_solution.feasible
    assert source_solution.objective == pytest.approx(-6)
    assert source_solution.state.entries == pytest.approx({0: 0, 1: 3})


def test_identity_only_policy_rejects_lowering_without_mutating_source() -> None:
    source = _one_hot_instance()
    before = source.to_v2_bytes()
    input_class = OMMXHighsAdapter.INPUT_CLASS
    assert input_class is not None
    policy = PreparationPolicy(acceptable_instance_class=input_class)

    with pytest.raises(PreparationError) as error:
        source.prepare(policy)

    failure = error.value.failure
    assert isinstance(failure, PreparationFailure.TargetInstanceClassNotReached)
    assert failure.current_membership.is_member is False
    assert failure.transforms == []
    assert source.to_v2_bytes() == before
