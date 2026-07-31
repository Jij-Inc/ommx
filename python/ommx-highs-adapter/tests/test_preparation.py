import pytest

from ommx import (
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    Instance,
    OneHotConstraint,
    Sense,
    Solution,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx.adapter import PreparationError, PreparationPolicy

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


@pytest.mark.parametrize(
    ("make_source", "kind", "transform_name"),
    [
        (_indicator_instance, SpecialConstraintKind.Indicator, "indicator_lowering"),
        (_one_hot_instance, SpecialConstraintKind.OneHot, "one_hot_lowering"),
        (_sos1_instance, SpecialConstraintKind.Sos1, "sos1_lowering"),
    ],
)
def test_prepare_lowers_declared_exact_special_constraint(
    make_source, kind, transform_name
):
    source = make_source()
    before = source.to_v2_bytes()

    preparation = OMMXHighsAdapter.prepare(source)

    assert OMMXHighsAdapter.PREPARATION_SPECIAL_CONSTRAINT_LOWERINGS == (
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    )
    assert preparation.source.to_v2_bytes() == before
    assert source.to_v2_bytes() == before
    assert kind not in preparation.input.active_special_constraint_kinds
    assert OMMXHighsAdapter.check_applicability(preparation.input).is_applicable
    assert [transform.name for transform in preparation.report.transforms] == [
        transform_name
    ]


def test_sos1_preparation_decodes_target_solution_to_source_solution():
    source = _sos1_instance()
    preparation = OMMXHighsAdapter.prepare(source)

    target_solution = OMMXHighsAdapter.solve(preparation.input)
    source_solution = preparation.decode(target_solution)

    assert source_solution.feasible
    assert source_solution.optimality == Solution.OPTIMAL
    assert source_solution.objective == pytest.approx(-6)
    assert source_solution.state.entries == pytest.approx({0: 0, 1: 3})


def test_empty_policy_rejects_lowering_without_mutating_source():
    source = _one_hot_instance()
    before = source.to_v2_bytes()
    policy = PreparationPolicy(allowed_special_constraint_lowerings=frozenset())

    with pytest.raises(PreparationError) as exc_info:
        OMMXHighsAdapter.prepare(source, policy=policy)

    [failure] = exc_info.value.report.preparation_failures
    assert failure.name == "special_constraint_lowering"
    assert failure.reason == (
        "preparation.policy.special_constraint_lowering_not_allowed"
    )
    assert exc_info.value.report.transforms == ()
    assert source.to_v2_bytes() == before
