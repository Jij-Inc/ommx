import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    InstanceClassMismatch,
    Kind,
    PolynomialRequirement,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import AdapterNotApplicableError, InfeasibleDetected

from ommx_highs_adapter import OMMXHighsAdapter, OMMXHighsAdapterError


def test_declares_linear_mip_input_class():
    input_class = OMMXHighsAdapter.INPUT_CLASS
    [clause] = input_class.clauses
    assert clause.label == "highs-linear-mip"
    assert clause.allowed_variable_kinds == {
        Kind.Binary,
        Kind.Integer,
        Kind.Continuous,
    }
    assert clause.objective_polynomial_requirement == PolynomialRequirement.at_most(1)
    assert clause.regular_constraint_polynomial_requirements == {
        Equality.EqualToZero: PolynomialRequirement.at_most(1),
        Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(1),
    }
    assert clause.indicator_body_polynomial_requirements == {}
    assert not clause.allows_one_hot
    assert not clause.allows_sos1
    assert clause.allowed_senses == {Sense.Minimize, Sense.Maximize}


@pytest.mark.parametrize("sense", [Sense.Minimize, Sense.Maximize])
def test_input_class_accepts_complete_linear_mip_boundary(sense):
    x = DecisionVariable.binary(0)
    y = DecisionVariable.integer(1)
    z = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        decision_variables=[x, y, z],
        objective=x + y + z,
        constraints={0: x + y == 1, 1: z <= 1},
        sense=sense,
    )

    report = OMMXHighsAdapter.check_applicability(instance)
    assert report.is_member
    assert report.matching_clauses == [(0, "highs-linear-mip")]


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(ommx_instance)
    mismatches = e.value.report.clause_reports[0].mismatches
    assert len(mismatches) == 1
    assert isinstance(
        mismatches[0],
        InstanceClassMismatch.ObjectiveDegreeExceedsBound,
    )


def test_error_non_polynomial_objective():
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=abs(Function(x)),
        constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as error:
        OMMXHighsAdapter(instance)

    [mismatch] = error.value.report.clause_reports[0].mismatches
    assert isinstance(mismatch, InstanceClassMismatch.ObjectiveFunctionNotPolynomial)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,  # constant 0
        constraints={0: 2.3 * x * x == 0},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(ommx_instance)
    mismatches = e.value.report.clause_reports[0].mismatches
    assert len(mismatches) == 1
    assert isinstance(
        mismatches[0],
        InstanceClassMismatch.RegularConstraintDegreeExceedsBound,
    )


@pytest.mark.parametrize(
    ("variable", "kind"),
    [
        (DecisionVariable.semi_integer(0, lower=1, upper=3), Kind.SemiInteger),
        (
            DecisionVariable.semi_continuous(0, lower=1, upper=3),
            Kind.SemiContinuous,
        ),
    ],
)
def test_rejects_unsupported_variable_kinds(variable, kind):
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(instance)
    mismatches = e.value.report.clause_reports[0].mismatches
    assert len(mismatches) == 1
    mismatch = mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.VariableKindNotAllowed)
    assert mismatch.kind == kind
    assert mismatch.variable_ids == {0}


def test_accepts_unused_unsupported_variable_kind_without_mutating_input():
    used = DecisionVariable.binary(0)
    unused = DecisionVariable.semi_integer(1, lower=1, upper=3)
    instance = Instance.from_components(
        decision_variables=[used, unused],
        objective=used,
        constraints={},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    report = OMMXHighsAdapter.check_applicability(instance)
    assert report.is_member
    assert report.matching_clauses == [(0, "highs-linear-mip")]
    OMMXHighsAdapter(instance)
    assert instance.to_v2_bytes() == before


def test_rejects_special_constraints_without_mutating_input():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.continuous(1, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        sense=Sense.Minimize,
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={20: OneHotConstraint(variables=[x])},
        sos1_constraints={30: Sos1Constraint(variables=[y])},
    )
    before = instance.to_v2_bytes()

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(instance)

    mismatch_types = {
        type(mismatch) for mismatch in e.value.report.clause_reports[0].mismatches
    }
    assert InstanceClassMismatch.IndicatorConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.OneHotConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.Sos1ConstraintsNotAllowed in mismatch_types
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterNotApplicableError):
        OMMXHighsAdapter.solve_without_preparation(instance)
    assert instance.to_v2_bytes() == before

    solution = OMMXHighsAdapter.solve(instance)
    assert solution.feasible
    assert instance.to_v2_bytes() == before


def test_recommended_preparation_reaches_the_highs_input_class():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.continuous(1, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        sense=Sense.Minimize,
        indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={20: OneHotConstraint(variables=[x])},
        sos1_constraints={30: Sos1Constraint(variables=[y])},
    )
    input_class = OMMXHighsAdapter.INPUT_CLASS
    assert not input_class.contains(instance)

    policy = OMMXHighsAdapter.recommended_preparation_policy()
    assert policy.special_constraints is not None
    assert policy.objective is None
    assert policy.integer_slack is None
    assert policy.integer_encoding is None
    assert policy.fixed_penalty is None

    assert instance.prepare(input_class, policy) is None
    assert input_class.contains(instance)
    assert OMMXHighsAdapter.check_applicability(instance).is_member


def test_error_infeasible_constant_equality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={
            0: Constraint(
                function=-1,
                equality=Equality.EqualToZero,
            )
        },
        sense=Sense.Minimize,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant equality constraint" in str(e.value)


def test_error_infeasible_constant_inequality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={
            0: Constraint(
                function=1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        sense=Sense.Minimize,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant inequality constraint" in str(e.value)


def test_error_infeasible_model():
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints={
            0: Constraint(
                function=x,
                equality=Equality.EqualToZero,
            ),
            1: Constraint(
                function=x - 1,
                equality=Equality.EqualToZero,
            ),
        },
        sense=Sense.Minimize,
    )
    with pytest.raises(InfeasibleDetected):
        OMMXHighsAdapter.solve(ommx_instance)
