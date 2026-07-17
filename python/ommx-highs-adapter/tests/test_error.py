import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    DegreeBound,
    Equality,
    IndicatorConstraint,
    Instance,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import AdapterNotApplicableError, InfeasibleDetected

from ommx_highs_adapter import OMMXHighsAdapter, OMMXHighsAdapterError


def test_declares_linear_mip_input_class():
    input_class = OMMXHighsAdapter.INPUT_CLASS
    assert input_class is not None
    [clause] = input_class.clauses
    assert clause.label == "highs-linear-mip"
    assert clause.allowed_variable_kinds == {
        Kind.Binary,
        Kind.Integer,
        Kind.Continuous,
    }
    assert clause.objective_degree_bound == DegreeBound.at_most(1)
    assert clause.regular_constraint_degree_bounds == {
        Equality.EqualToZero: DegreeBound.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
    }
    assert clause.indicator_constraint_degree_bounds == {}
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
    assert report.is_applicable
    assert report.input_membership.matching_clauses == [(0, "highs-linear-mip")]
    assert report.precondition_violations == ()


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(ommx_instance)
    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
    assert len(mismatches) == 1
    assert isinstance(
        mismatches[0],
        InstanceClassMismatch.ObjectiveDegreeExceedsBound,
    )


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,  # constant 0
        constraints={0: 2.3 * x * x == 0},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXHighsAdapter(ommx_instance)
    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
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
    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
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
    assert report.is_applicable
    assert report.input_membership.matching_clauses == [(0, "highs-linear-mip")]
    OMMXHighsAdapter(instance)
    assert instance.to_v2_bytes() == before


def test_rejects_special_constraints_without_mutating_input():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.continuous(1)
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
        type(mismatch)
        for mismatch in e.value.report.input_membership.clause_reports[0].mismatches
    }
    assert InstanceClassMismatch.IndicatorConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.OneHotConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.Sos1ConstraintsNotAllowed in mismatch_types
    assert instance.to_v2_bytes() == before


def test_error_infeasible_constant_equality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={
            0: Constraint(
                function=-1,
                equality=Constraint.EQUAL_TO_ZERO,
            )
        },
        sense=Instance.MINIMIZE,
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
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            )
        },
        sense=Instance.MINIMIZE,
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
                equality=Constraint.EQUAL_TO_ZERO,
            ),
            1: Constraint(
                function=x - 1,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        },
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(InfeasibleDetected):
        OMMXHighsAdapter.solve(ommx_instance)
