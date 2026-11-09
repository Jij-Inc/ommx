import pytest

from ommx import (
    DecisionVariable,
    Equality,
    Function,
    IndicatorConstraint,
    Instance,
    InstanceClassMismatch,
    Kind,
    OneHotConstraint,
    PolynomialRequirement,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import AdapterNotApplicableError
from ommx_python_mip_adapter import OMMXPythonMIPAdapter


def test_declares_linear_mip_input_class() -> None:
    input_class = OMMXPythonMIPAdapter.INPUT_CLASS
    [clause] = input_class.clauses

    assert clause.label == "python-mip-linear-mip"
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
def test_input_class_accepts_complete_linear_mip_boundary(sense: Sense) -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.integer(1)
    z = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        decision_variables=[x, y, z],
        objective=x + y + z,
        active_constraints={0: x + y == 1, 1: z <= 1},
        sense=sense,
    )

    report = OMMXPythonMIPAdapter.check_applicability(instance)

    assert report.is_member
    assert report.matching_clauses == [(0, "python-mip-linear-mip")]


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x (variable ID should match)
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        active_constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert isinstance(
        e.value.report.clause_reports[0].mismatches[0],
        InstanceClassMismatch.ObjectiveDegreeExceedsBound,
    )


def test_error_non_polynomial_objective() -> None:
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=abs(Function(x)),
        active_constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as error:
        OMMXPythonMIPAdapter(instance)

    [mismatch] = error.value.report.clause_reports[0].mismatches
    assert isinstance(mismatch, InstanceClassMismatch.ObjectiveFunctionNotPolynomial)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0.0,
        active_constraints={0: 2.3 * x * x == 0},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert isinstance(
        e.value.report.clause_reports[0].mismatches[0],
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
def test_rejects_used_unsupported_variable_kinds(
    variable: DecisionVariable, kind: Kind
) -> None:
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        active_constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPythonMIPAdapter(instance)

    mismatch = e.value.report.clause_reports[0].mismatches[0]
    assert isinstance(mismatch, InstanceClassMismatch.VariableKindNotAllowed)
    assert mismatch.kind == kind
    assert mismatch.variable_ids == {0}


def test_ignores_unused_unsupported_variable_kind() -> None:
    used = DecisionVariable.continuous(0)
    unused = DecisionVariable.semi_integer(1, lower=1, upper=3)
    instance = Instance.from_components(
        decision_variables=[used, unused],
        objective=used,
        active_constraints={},
        sense=Sense.Minimize,
    )

    report = OMMXPythonMIPAdapter.check_applicability(instance)
    adapter = OMMXPythonMIPAdapter(instance)

    assert report.is_member
    assert adapter.instance is instance
    assert [variable.name for variable in adapter.solver_input.vars] == ["0"]


def test_rejects_special_constraints_without_mutating_input() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.continuous(1, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        active_constraints={},
        sense=Sense.Minimize,
        active_indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        active_one_hot_constraints={20: OneHotConstraint(variables=[x])},
        active_sos1_constraints={30: Sos1Constraint(variables=[y])},
    )
    before = instance.to_v2_bytes()

    with pytest.raises(AdapterNotApplicableError) as e:
        OMMXPythonMIPAdapter(instance)

    mismatches = e.value.report.clause_reports[0].mismatches
    mismatch_types = {type(mismatch) for mismatch in mismatches}
    assert InstanceClassMismatch.IndicatorConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.OneHotConstraintsNotAllowed in mismatch_types
    assert InstanceClassMismatch.Sos1ConstraintsNotAllowed in mismatch_types
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterNotApplicableError):
        OMMXPythonMIPAdapter.solve_without_preparation(instance)
    assert instance.to_v2_bytes() == before

    solution = OMMXPythonMIPAdapter.solve(instance)
    assert solution.feasible()
    assert instance.to_v2_bytes() == before


def test_recommended_preparation_reaches_the_python_mip_input_class() -> None:
    x = DecisionVariable.binary(0)
    y = DecisionVariable.continuous(1, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        active_constraints={},
        sense=Sense.Minimize,
        active_indicator_constraints={
            10: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        active_one_hot_constraints={20: OneHotConstraint(variables=[x])},
        active_sos1_constraints={30: Sos1Constraint(variables=[y])},
    )
    input_class = OMMXPythonMIPAdapter.INPUT_CLASS
    assert not input_class.contains(instance)

    policy = OMMXPythonMIPAdapter.recommended_preparation_policy()
    assert policy.special_constraints is not None
    assert policy.objective is None
    assert policy.integer_slack is None
    assert policy.integer_encoding is None
    assert policy.fixed_penalty is None

    assert instance.prepare(input_class, policy) is None
    assert input_class.contains(instance)
    assert OMMXPythonMIPAdapter.check_applicability(instance).is_member


def test_dual_mapping_preserves_typed_sparse_constraint_ids():
    x = DecisionVariable.continuous(0, lower=0, upper=10)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        sense=Sense.Maximize,
        active_constraints={42: x <= 2},
    )
    adapter = OMMXPythonMIPAdapter(instance)
    model = adapter.solver_input
    model.optimize()
    solution = adapter.decode(model)
    assert solution.get_dual_variable(42) == pytest.approx(1)


@pytest.mark.parametrize("skip_feasibility_check", [False, True])
def test_inactive_constraints_are_evaluated_but_not_imposed(skip_feasibility_check):
    from ommx import ConstraintActiveness

    instance = Instance.minimize()
    x = instance.new_continuous("x", lower=0, upper=2)
    instance.objective = x
    row = instance.add_constraint(x >= 1, "retained_requirement")
    instance.deactivate_constraint(
        row.constraint_id, "manual", skip_feasibility_check=skip_feasibility_check
    )

    solution = OMMXPythonMIPAdapter.solve_without_preparation(instance)

    assert solution.state.entries[x.id] == pytest.approx(0)
    assert solution.constraint_violation(row.constraint_id) == pytest.approx(1)
    assert solution.feasible_relaxed
    assert solution.feasible() is skip_feasibility_check
    assert not solution.feasible(
        min_activeness=ConstraintActiveness.SKIP_FEASIBILITY_CHECK
    )
