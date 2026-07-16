import pytest
import pyscipopt

from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapterError,
    OMMXPySCIPOptAdapter,
)

from ommx.adapter import AdapterNotApplicableError, InfeasibleDetected
from ommx import (
    Constraint,
    DecisionVariable,
    DegreeBound,
    Equality,
    Instance,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    Polynomial,
    Sense,
    Sos1Constraint,
)


def test_declares_quadratic_mip_input_class():
    input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
    assert input_class is not None
    [clause] = input_class.clauses
    assert clause.label == "pyscipopt-quadratic-mip"
    assert clause.allowed_variable_kinds == {
        Kind.Binary,
        Kind.Integer,
        Kind.Continuous,
    }
    assert clause.objective_degree_bound == DegreeBound.at_most(2)
    assert clause.regular_constraint_degree_bounds == {
        Equality.EqualToZero: DegreeBound.at_most(2),
        Equality.LessThanOrEqualToZero: DegreeBound.at_most(2),
    }
    assert clause.indicator_constraint_degree_bounds == {
        Equality.EqualToZero: DegreeBound.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
    }
    assert not clause.allows_one_hot
    assert clause.allows_sos1
    assert clause.allowed_senses == {Sense.Minimize, Sense.Maximize}


@pytest.mark.parametrize("sense", [Sense.Minimize, Sense.Maximize])
def test_input_class_accepts_complete_quadratic_mip_boundary(sense):
    b = DecisionVariable.binary(0)
    x = DecisionVariable.integer(1)
    y = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        decision_variables=[b, x, y],
        objective=x * x + y * y,
        constraints={0: x * x <= 9, 1: y * y == 1},
        indicator_constraints={
            10: (y <= 2).with_indicator(b),
            11: (y == 1).with_indicator(b),
        },
        sos1_constraints={20: Sos1Constraint(variables=[x, y])},
        sense=sense,
    )
    before = instance.to_v2_bytes()

    report = OMMXPySCIPOptAdapter.check_applicability(instance)
    assert report.is_applicable
    assert report.input_membership.matching_clauses == [(0, "pyscipopt-quadratic-mip")]
    assert report.precondition_violations == ()
    OMMXPySCIPOptAdapter(instance)
    assert instance.to_v2_bytes() == before


def test_error_polynomial_objective():
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints={},
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
    assert len(mismatches) == 1
    mismatch = mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.ObjectiveDegreeExceedsBound)
    assert mismatch.actual_degree == 3
    assert mismatch.bound == DegreeBound.at_most(2)


def test_rejects_inapplicable_input_before_backend_construction(monkeypatch):
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x * x * x,
        constraints={},
        sense=Sense.Minimize,
    )

    def unexpected_model_construction(*args, **kwargs):
        pytest.fail("PySCIPOpt model was constructed for an inapplicable input")

    monkeypatch.setattr(pyscipopt, "Model", unexpected_model_construction)

    with pytest.raises(AdapterNotApplicableError):
        OMMXPySCIPOptAdapter(instance)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x * x = 0
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=0,
        constraints={
            0: Constraint(
                function=Polynomial(terms={(1, 1, 1): 2.3}),
                equality=Constraint.EQUAL_TO_ZERO,
            )
        },
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
    assert len(mismatches) == 1
    mismatch = mismatches[0]
    assert isinstance(
        mismatch, InstanceClassMismatch.RegularConstraintDegreeExceedsBound
    )
    assert mismatch.actual_degrees == {0: 3}
    assert mismatch.bound == DegreeBound.at_most(2)


def test_rejects_quadratic_indicator_body_without_mutating_input():
    b = DecisionVariable.binary(0)
    x = DecisionVariable.continuous(1)
    instance = Instance.from_components(
        decision_variables=[b, x],
        objective=x,
        constraints={},
        indicator_constraints={0: (x * x <= 1).with_indicator(b)},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPySCIPOptAdapter(instance)

    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
    assert len(mismatches) == 1
    mismatch = mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.IndicatorBodyDegreeExceedsBound)
    assert mismatch.actual_degrees == {0: 2}
    assert mismatch.bound == DegreeBound.at_most(1)
    assert instance.to_v2_bytes() == before


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
        OMMXPySCIPOptAdapter(instance)
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

    report = OMMXPySCIPOptAdapter.check_applicability(instance)
    assert report.is_applicable
    OMMXPySCIPOptAdapter(instance)
    assert instance.to_v2_bytes() == before


def test_rejects_one_hot_without_implicit_lowering():
    x = DecisionVariable.binary(0)
    y = DecisionVariable.binary(1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={},
        one_hot_constraints={0: OneHotConstraint(variables=[x, y])},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPySCIPOptAdapter(instance)

    mismatches = e.value.report.input_membership.clause_reports[0].mismatches
    assert len(mismatches) == 1
    mismatch = mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.OneHotConstraintsNotAllowed)
    assert mismatch.constraint_ids == {0}
    assert instance.to_v2_bytes() == before


def test_error_not_optimized_model():
    model = pyscipopt.Model()
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(instance).decode_to_state(model)
    assert "The model may not be optimized." in str(e.value)


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
        OMMXPySCIPOptAdapter.solve(ommx_instance)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)
