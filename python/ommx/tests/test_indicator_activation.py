"""Activation conditions survive evaluation, transformation, and SDK boundaries."""

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


def model(zero: bool, equality: Equality = Equality.EqualToZero) -> Instance:
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        constraints={},
        indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=z,
                function=x - 2,
                equality=equality,
                activate_on_zero=zero,
            )
        },
    )


@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize(
    "equality", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
def test_lifecycle_and_bigm(zero, equality):
    source = model(zero, equality)
    cid = IndicatorConstraintID(7)
    attached = source.constraints[cid]
    assert attached.activate_on_zero == zero
    for row in [
        attached.detach(),
        copy.copy(attached.detach()),
        copy.deepcopy(attached.detach()),
    ]:
        assert row.activate_on_zero == zero
    assert (
        source.constraints_df(kind=ConstraintKind.Indicator).loc[
            cid, "activate_on_zero"
        ]
        == zero
    )
    restored = Instance.from_v2_bytes(source.to_v2_bytes())
    assert restored.constraints[cid].activate_on_zero == zero
    assert f"= {int(not zero)} ->" in str(source)

    samples = source.evaluate_samples(
        {
            10: {0: 0, 1: 3},
            20: {0: 1, 1: 3},
        }
    )
    for result in [samples, SampleSet.from_v2_bytes(samples.to_v2_bytes())]:
        row = result.constraints[cid]
        assert row.activate_on_zero == zero
        assert row.indicator_active == {10: zero, 20: not zero}
        assert (
            result.constraints_df(kind=ConstraintKind.Indicator).loc[
                cid, "activate_on_zero"
            ]
            == zero
        )
        for sample_id, active in [(10, zero), (20, not zero)]:
            solution = result.get(sample_id)
            for evaluated in [solution, Solution.from_v2_bytes(solution.to_v2_bytes())]:
                assert evaluated.constraints[cid].activate_on_zero == zero
                assert evaluated.constraints[cid].indicator_active == active
                assert evaluated.feasible == (not active)
                assert (
                    evaluated.constraints_df(kind=ConstraintKind.Indicator).loc[
                        cid, "activate_on_zero"
                    ]
                    == zero
                )

    lowered = copy.deepcopy(source)
    lowered.convert_all_indicators_to_constraints()
    removed = lowered.removed_constraints[cid]
    assert removed.activate_on_zero == zero
    assert removed.constraint.activate_on_zero == zero
    assert (
        Instance.from_v2_bytes(lowered.to_v2_bytes())
        .removed_constraints[cid]
        .activate_on_zero
        == zero
    )
    for z in [0, 1]:
        for x in [-5, 0, 2, 5]:
            state = {0: z, 1: x}
            active = z == int(not zero)
            expected = not active or (
                x == 2 if equality == Equality.EqualToZero else x <= 2
            )
            assert source.evaluate(state).feasible == expected
            assert lowered.evaluate(state).feasible == expected
            assert lowered.evaluate(state).feasible_relaxed == expected


@pytest.mark.parametrize("zero", [False, True])
def test_with_indicator_and_parameter_materialization(zero):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    p = Parameter(id=100, name="target")
    assert not (x <= 2).with_indicator(z).activate_on_zero
    row = (x == p).with_indicator(z, activate_on_zero=zero)
    assert row.activate_on_zero == zero
    parametric = ParametricInstance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        parameters=[p],
        constraints={},
        indicator_constraints={7: row},
    )
    cid = IndicatorConstraintID(7)
    for host in [
        parametric,
        ParametricInstance.from_v2_bytes(parametric.to_v2_bytes()),
    ]:
        assert host.constraints[cid].activate_on_zero == zero
        assert host.constraints[cid].detach().activate_on_zero == zero
        concrete = host.with_parameters({100: 2})
        assert concrete.constraints[cid].activate_on_zero == zero
        assert not concrete.evaluate({0: int(not zero), 1: 3}).feasible
        assert concrete.evaluate({0: int(zero), 1: 3}).feasible


def input_class(values, requirements=None):
    return InstanceClass(
        [
            InstanceClassClause(
                label="indicator activation",
                allowed_variable_kinds={Kind.Binary, Kind.Continuous},
                allowed_senses={Sense.Minimize},
                objective_polynomial_requirement=PolynomialRequirement.at_most(1),
                allowed_indicator_activation_values=None
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


@pytest.mark.parametrize("zero", [False, True])
def test_membership_distinguishes_activation(zero):
    source = model(zero)
    before = source.to_v2_bytes()
    assert input_class([int(not zero)]).contains(source)
    assert input_class([0, 1]).contains(source)
    report = input_class([int(zero)]).check_membership(source)
    assert not report.is_member
    mismatch = report.clause_reports[0].mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.IndicatorActivationNotAllowed)
    assert mismatch.activation_value == int(not zero)
    assert mismatch.allowed_activation_values == {int(zero)}
    assert mismatch.constraint_ids == {IndicatorConstraintID(7)}
    assert source.to_v2_bytes() == before
    assert input_class([int(not zero)]).clauses[
        0
    ].indicator_body_polynomial_requirements == {
        Equality.EqualToZero: PolynomialRequirement.at_most(1)
    }
    source.add_indicator_constraint(
        (DecisionVariable.continuous(1) == 2).with_indicator(
            0, activate_on_zero=not zero
        )
    )
    assert not input_class([int(not zero)]).contains(source)
    assert input_class([0, 1]).contains(source)


@pytest.mark.parametrize("values", [[], [0], [1], [0, 1]])
@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize(
    "relation", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize(
    "allowed_relation", [Equality.EqualToZero, Equality.LessThanOrEqualToZero]
)
@pytest.mark.parametrize("bound", [0, 1])
def test_activation_and_body_requirements_are_orthogonal(
    values, zero, relation, allowed_relation, bound
):
    target = input_class(
        values, {allowed_relation: PolynomialRequirement.at_most(bound)}
    )
    source = model(zero, relation)
    before = source.to_v2_bytes()
    assert target.contains(source) == (
        int(not zero) in values and relation == allowed_relation and bound >= 1
    )
    assert source.to_v2_bytes() == before


def test_activation_defaults_empty_sets_and_independent_diagnostics():
    target = input_class(None)
    assert target.clauses[0].allowed_indicator_activation_values == {1}
    assert target.contains(model(False))
    assert not target.contains(model(True))
    assert not input_class([0, 1], {}).contains(model(False))
    assert not input_class([0, 1], {}).contains(model(True))
    target = input_class([], {Equality.EqualToZero: PolynomialRequirement.at_most(0)})
    mismatches = target.check_membership(model(True)).clause_reports[0].mismatches
    assert len(mismatches) == 2
    assert isinstance(
        mismatches[0], InstanceClassMismatch.IndicatorActivationNotAllowed
    )
    assert mismatches[0].activation_value == 0
    assert mismatches[0].allowed_activation_values == set()
    assert isinstance(
        mismatches[1], InstanceClassMismatch.IndicatorBodyDegreeExceedsBound
    )
    assert mismatches[1].actual_degrees == {IndicatorConstraintID(7): 1}
    empty = Instance.from_components(
        sense=Sense.Minimize, objective=0, decision_variables=[], constraints={}
    )
    assert target.contains(empty)


@pytest.mark.parametrize("values", [[2], [0, 255]])
def test_invalid_activation_values_are_rejected(values):
    with pytest.raises(RuntimeError, match="must be 0 or 1"):
        input_class(values)


@pytest.mark.parametrize("zero", [False, True])
@pytest.mark.parametrize("active", [False, True])
def test_fixed_trigger_propagates_and_preserves_removed_condition(zero, active):
    source = model(zero)
    value = int(not zero) if active else int(zero)
    source = source.partial_evaluate({0: value})
    assert IndicatorConstraintID(7) not in source.constraints
    removed = source.removed_constraints[IndicatorConstraintID(7)]
    assert removed.activate_on_zero == zero
    assert source.evaluate({1: 3}).feasible == (not active)
    assert source.evaluate({1: 3}).feasible_relaxed == (not active)
    restored = Instance.from_v2_bytes(source.to_v2_bytes())
    assert (
        restored.removed_constraints[IndicatorConstraintID(7)].activate_on_zero == zero
    )
    assert restored.evaluate({1: 3}).feasible == (not active)
    assert restored.evaluate({1: 3}).feasible_relaxed == (not active)


@pytest.mark.parametrize("zero", [False, True])
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
def test_bigm_active_formulation_with_trigger_in_body(zero, equality, a, b, c, d):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1, lower=-5, upper=5)
    body = a * x * x + b * x + c * z + d
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        constraints={},
        indicator_constraints={
            7: IndicatorConstraint(
                indicator_variable=z,
                function=body,
                equality=equality,
                activate_on_zero=zero,
            )
        },
    )
    lowered = copy.deepcopy(source)
    lowered.convert_all_indicators_to_constraints()
    for zv in [0, 1]:
        for xv in [-5, -1, 0, 1, 5]:
            residual = a * xv * xv + b * xv + c * zv + d
            expected = zv != int(not zero) or (
                residual == 0 if equality == Equality.EqualToZero else residual <= 0
            )
            state = {0: zv, 1: xv}
            assert source.evaluate(state).feasible == expected
            # Removed indicators must not hide a missing or weakened Big-M side.
            assert lowered.evaluate(state).feasible_relaxed == expected


@pytest.mark.parametrize("zero", [False, True])
def test_zero_activation_bigm_failure_is_atomic(zero):
    z = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    source = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[z, x],
        constraints={},
        indicator_constraints={7: (x <= 2).with_indicator(z, activate_on_zero=zero)},
    )
    before = source.to_v2_bytes()
    with pytest.raises(RuntimeError, match="non-finite"):
        source.convert_all_indicators_to_constraints()
    assert source.to_v2_bytes() == before
