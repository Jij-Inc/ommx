"""Trigger values survive evaluation, transformation, and SDK boundaries."""

import copy

import pytest

from ommx import (
    ConstraintKind,
    DecisionVariable,
    Equality,
    IndicatorConstraint,
    IndicatorConstraintID,
    Instance,
    InstanceClass,
    InstanceClassClause,
    InstanceClassMismatch,
    Kind,
    Parameter,
    ParametricInstance,
    PolynomialRequirement,
    SampleSet,
    Sense,
    Solution,
)


def model(trigger_value: int, equality: Equality = Equality.EqualToZero) -> Instance:
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        active_constraints={},
        active_indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=z,
                function=x - 2,
                equality=equality,
                trigger_value=trigger_value,
            )
        },
    )


@pytest.mark.parametrize("trigger_value", [0, 1])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
def test_lifecycle_and_bigm(trigger_value, equality):
    source = model(trigger_value, equality)
    cid = IndicatorConstraintID(7)
    attached = source.active_constraints[cid]
    assert attached.trigger_value == trigger_value
    assert type(attached.trigger_value) is int
    for row in [
        attached.detach(),
        copy.copy(attached.detach()),
        copy.deepcopy(attached.detach()),
    ]:
        assert row.trigger_value == trigger_value
        assert type(row.trigger_value) is int
    assert (
        source.constraints_df(kind=ConstraintKind.Indicator).loc[cid, "trigger_value"]
        == trigger_value
    )
    restored = Instance.from_v2_bytes(source.to_v2_bytes())
    assert restored.active_constraints[cid].trigger_value == trigger_value
    assert f"= {trigger_value} ->" in str(source)

    samples = source.evaluate_samples(
        {
            10: {0: 0, 1: 3},
            20: {0: 1, 1: 3},
        }
    )
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        row = result.constraints[cid]
        assert row.trigger_value == trigger_value
        assert type(row.trigger_value) is int
        assert row.indicator_on == {10: trigger_value == 0, 20: trigger_value == 1}
        assert all(type(on) is bool for on in row.indicator_on.values())
        assert (
            result.constraints_df(kind=ConstraintKind.Indicator).loc[
                cid, "trigger_value"
            ]
            == trigger_value
        )
        for sample_id, indicator_on in [
            (10, trigger_value == 0),
            (20, trigger_value == 1),
        ]:
            solution = result.get(sample_id)
            for evaluated in [solution, Solution.from_v2_bytes(solution.to_v2_bytes())]:
                assert evaluated.constraints[cid].trigger_value == trigger_value
                assert type(evaluated.constraints[cid].trigger_value) is int
                assert evaluated.constraints[cid].indicator_on is indicator_on
                assert evaluated.feasible == (not indicator_on)
                assert (
                    evaluated.constraints_df(kind=ConstraintKind.Indicator).loc[
                        cid, "trigger_value"
                    ]
                    == trigger_value
                )

    lowered = copy.deepcopy(source)
    lowered.convert_all_indicators_to_constraints()
    inactive = lowered.inactive_constraints[cid]
    assert inactive.trigger_value == trigger_value
    assert type(inactive.trigger_value) is int
    assert inactive.constraint.trigger_value == trigger_value
    assert type(inactive.constraint.trigger_value) is int
    assert (
        Instance.from_v2_bytes(lowered.to_v2_bytes())
        .inactive_constraints[cid]
        .trigger_value
        == trigger_value
    )
    for z in [0, 1]:
        for x in [-5, 0, 2, 5]:
            state = {0: z, 1: x}
            indicator_on = z == trigger_value
            expected = not indicator_on or (
                x == 2 if equality == Equality.EqualToZero else x <= 2
            )
            assert source.evaluate(state).feasible == expected
            assert lowered.evaluate(state).feasible == expected
            assert lowered.evaluate(state).feasible_relaxed == expected


@pytest.mark.parametrize("trigger_value", [0, 1])
def test_with_indicator_and_parameter_materialization(trigger_value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    p = Parameter(id=100, name="target")
    assert (x <= 2).with_indicator(z).trigger_value == 1
    row = (x == p).with_indicator(z, trigger_value=trigger_value)
    assert row.trigger_value == trigger_value
    assert type(row.trigger_value) is int
    parametric = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        parameters=[p],
        active_constraints={},
        active_indicator_constraints={7: row},
    )
    cid = IndicatorConstraintID(7)
    for host in [
        parametric,
        ParametricInstance.from_v2_bytes(parametric.to_v2_bytes()),
    ]:
        assert host.active_constraints[cid].trigger_value == trigger_value
        assert type(host.active_constraints[cid].trigger_value) is int
        assert host.active_constraints[cid].detach().trigger_value == trigger_value
        concrete = host.with_parameters({100: 2})
        assert concrete.active_constraints[cid].trigger_value == trigger_value
        assert not concrete.evaluate({0: trigger_value, 1: 3}).feasible
        assert concrete.evaluate({0: 1 - trigger_value, 1: 3}).feasible


def input_class(values, requirements=None):
    return InstanceClass(
        [
            InstanceClassClause(
                label="indicator trigger value",
                allowed_variable_kinds={Kind.Binary, Kind.Continuous},
                allowed_senses={Sense.Minimize},
                objective_polynomial_requirement=PolynomialRequirement.at_most(1),
                allowed_indicator_trigger_values=None
                if values is None
                else set(values),
                indicator_body_polynomial_requirements=(
                    requirements
                    if requirements is not None
                    else {Equality.EqualToZero: PolynomialRequirement.at_most(1)}
                ),
            )
        ]
    )


@pytest.mark.parametrize("trigger_value", [0, 1])
def test_membership_distinguishes_trigger_value(trigger_value):
    source = model(trigger_value)
    before = source.to_v2_bytes()
    assert input_class([trigger_value]).contains(source)
    assert input_class([0, 1]).contains(source)
    report = input_class([1 - trigger_value]).check_membership(source)
    assert not report.is_member
    mismatch = report.clause_reports[0].mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.IndicatorTriggerNotAllowed)
    assert mismatch.trigger_value == trigger_value
    assert mismatch.allowed_trigger_values == {1 - trigger_value}
    assert mismatch.constraint_ids == {IndicatorConstraintID(7)}
    assert source.to_v2_bytes() == before
    assert input_class([trigger_value]).clauses[
        0
    ].indicator_body_polynomial_requirements == {
        Equality.EqualToZero: PolynomialRequirement.at_most(1)
    }
    source.add_indicator_constraint(
        (DecisionVariable.continuous(1) == 2).with_indicator(
            0, trigger_value=1 - trigger_value
        )
    )
    assert not input_class([trigger_value]).contains(source)
    assert input_class([0, 1]).contains(source)


@pytest.mark.parametrize("values", [[], [0], [1], [0, 1]])
@pytest.mark.parametrize("trigger_value", [0, 1])
@pytest.mark.parametrize(
    "relation", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize(
    "allowed_relation", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("bound", [0, 1])
def test_trigger_value_and_body_requirements_are_orthogonal(
    values, trigger_value, relation, allowed_relation, bound
):
    target = input_class(
        values, {allowed_relation: PolynomialRequirement.at_most(bound)}
    )
    source = model(trigger_value, relation)
    before = source.to_v2_bytes()
    assert target.contains(source) == (
        trigger_value in values and relation == allowed_relation and bound >= 1
    )
    assert source.to_v2_bytes() == before


def test_trigger_value_defaults_empty_sets_and_independent_diagnostics():
    target = input_class(None)
    assert target.clauses[0].allowed_indicator_trigger_values == {1}
    assert target.contains(model(1))
    assert not target.contains(model(0))
    assert not input_class([0, 1], {}).contains(model(1))
    assert not input_class([0, 1], {}).contains(model(0))
    target = input_class([], {Equality.EqualToZero: PolynomialRequirement.at_most(0)})
    mismatches = target.check_membership(model(0)).clause_reports[0].mismatches
    assert len(mismatches) == 2
    assert isinstance(mismatches[0], InstanceClassMismatch.IndicatorTriggerNotAllowed)
    assert mismatches[0].trigger_value == 0
    assert mismatches[0].allowed_trigger_values == set()
    assert isinstance(
        mismatches[1], InstanceClassMismatch.IndicatorBodyDegreeExceedsBound
    )
    assert mismatches[1].actual_degrees == {IndicatorConstraintID(7): 1}
    empty = Instance.from_components(
        sense=Sense.Minimize, objective=0, decision_variables=[], active_constraints={}
    )
    assert target.contains(empty)


@pytest.mark.parametrize("values", [[2], [0, 255]])
def test_invalid_allowed_trigger_values_are_rejected(values):
    with pytest.raises(RuntimeError, match="must be 0 or 1"):
        input_class(values)


def test_trigger_value_defaults_to_one():
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    indicator = IndicatorConstraint(
        indicator_variable=z,
        function=x - 2,
        equality=Equality.LessThanOrEqualToZero,
    )
    assert indicator.trigger_value == 1
    assert type(indicator.trigger_value) is int
    assert (x <= 2).with_indicator(z).trigger_value == 1


@pytest.mark.parametrize("trigger_value", [2, 255, 2**32 - 1])
def test_invalid_trigger_value_is_rejected_by_both_constructors(trigger_value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    with pytest.raises(RuntimeError, match="must be 0 or 1"):
        IndicatorConstraint(
            indicator_variable=z,
            function=x - 2,
            equality=Equality.LessThanOrEqualToZero,
            trigger_value=trigger_value,
        )
    with pytest.raises(RuntimeError, match="must be 0 or 1"):
        (x <= 2).with_indicator(z, trigger_value=trigger_value)


@pytest.mark.parametrize("trigger_value", [-1, 2**32])
def test_trigger_value_integer_overflow_is_rejected(trigger_value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    with pytest.raises(OverflowError):
        IndicatorConstraint(
            indicator_variable=z,
            function=x - 2,
            equality=Equality.LessThanOrEqualToZero,
            trigger_value=trigger_value,
        )
    with pytest.raises(OverflowError):
        (x <= 2).with_indicator(z, trigger_value=trigger_value)


@pytest.mark.parametrize("trigger_value", [0, 1])
@pytest.mark.parametrize("indicator_on", [False, True])
def test_fixed_trigger_propagates_and_preserves_inactive_condition(
    trigger_value, indicator_on
):
    source = model(trigger_value)
    value = trigger_value if indicator_on else 1 - trigger_value
    source = source.partial_evaluate({0: value})
    assert IndicatorConstraintID(7) not in source.active_constraints
    inactive = source.inactive_constraints[IndicatorConstraintID(7)]
    assert inactive.trigger_value == trigger_value
    assert source.evaluate({1: 3}).feasible == (not indicator_on)
    assert source.evaluate({1: 3}).feasible_relaxed == (not indicator_on)
    restored = Instance.from_v2_bytes(source.to_v2_bytes())
    assert (
        restored.inactive_constraints[IndicatorConstraintID(7)].trigger_value
        == trigger_value
    )
    assert restored.evaluate({1: 3}).feasible == (not indicator_on)
    assert restored.evaluate({1: 3}).feasible_relaxed == (not indicator_on)


@pytest.mark.parametrize("trigger_value", [0, 1])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize(
    "a,b,c,d",
    [
        (1, -2, 3, -4),
        (-1, 2, -3, 4),
        (0, 1, -2, 0),
        (0, 0, 1, -1),
        (1, 0, 1, 0),
        (-1, 0, -1, 0),
        (0, 0, 0, 0),
    ],
)
def test_bigm_active_formulation_with_trigger_in_body(
    trigger_value, equality, a, b, c, d
):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    body = a * x * x + b * x + c * z + d
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        active_constraints={},
        active_indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=z,
                function=body,
                equality=equality,
                trigger_value=trigger_value,
            )
        },
    )
    lowered = copy.deepcopy(source)
    lowered.convert_all_indicators_to_constraints()
    for zv in [0, 1]:
        for xv in [-5, -1, 0, 1, 5]:
            residual = a * xv * xv + b * xv + c * zv + d
            expected = zv != trigger_value or (
                residual == 0 if equality == Equality.EqualToZero else residual <= 0
            )
            state = {0: zv, 1: xv}
            assert source.evaluate(state).feasible == expected
            # Inactive indicators must not hide a missing or weakened Big-M side.
            assert lowered.evaluate(state).feasible_relaxed == expected


@pytest.mark.parametrize("trigger_value", [0, 1])
def test_trigger_value_bigm_failure_is_atomic(trigger_value):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        active_constraints={},
        active_indicator_constraints={
            7: (x <= 2).with_indicator(z, trigger_value=trigger_value)
        },
    )
    before = source.to_v2_bytes()
    with pytest.raises(RuntimeError, match="non-finite"):
        source.convert_all_indicators_to_constraints()
    assert source.to_v2_bytes() == before
