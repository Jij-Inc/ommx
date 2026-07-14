import pytest
import pyscipopt

from ommx_pyscipopt_adapter import (
    OMMXPySCIPOptAdapterError,
    OMMXPySCIPOptAdapter,
)

from ommx.adapter import AdapterCompatibilityError, InfeasibleDetected
from ommx import (
    Constraint,
    DecisionVariable,
    DegreeLimit,
    Equality,
    Instance,
    Kind,
    OneHotConstraint,
    Polynomial,
    PortableCapabilityMismatch,
    Sense,
    Sos1Constraint,
)


def test_declares_native_quadratic_capability_profile():
    capabilities = OMMXPySCIPOptAdapter.CAPABILITIES
    assert capabilities is not None
    [profile] = capabilities.profiles
    assert profile.name == "pyscipopt-quadratic"
    assert profile.variable_kinds == {Kind.Binary, Kind.Integer, Kind.Continuous}
    assert profile.objective_degree == DegreeLimit.at_most(2)
    assert profile.regular_constraints == {
        Equality.EqualToZero: DegreeLimit.at_most(2),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(2),
    }
    assert profile.indicator_constraints == {
        Equality.EqualToZero: DegreeLimit.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(1),
    }
    assert not profile.supports_one_hot
    assert profile.supports_sos1
    assert profile.senses == {Sense.Minimize, Sense.Maximize}


@pytest.mark.parametrize("sense", [Sense.Minimize, Sense.Maximize])
def test_capability_profile_accepts_complete_native_boundary(sense):
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

    report = OMMXPySCIPOptAdapter.check_compatibility(instance)
    assert report.compatible
    assert report.portable_report.matching_profiles == ["pyscipopt-quadratic"]


def test_error_polynomial_objective():
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints={},
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(mismatch, PortableCapabilityMismatch.ObjectiveDegreeExceeded)
    assert mismatch.actual_degree == 3
    assert mismatch.limit == DegreeLimit.at_most(2)


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
    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPySCIPOptAdapter(ommx_instance)
    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(
        mismatch, PortableCapabilityMismatch.RegularConstraintDegreeExceeded
    )
    assert mismatch.actual_degrees == {0: 3}
    assert mismatch.limit == DegreeLimit.at_most(2)


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

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPySCIPOptAdapter(instance)

    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(mismatch, PortableCapabilityMismatch.IndicatorBodyDegreeExceeded)
    assert mismatch.actual_degrees == {0: 2}
    assert mismatch.limit == DegreeLimit.at_most(1)
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

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPySCIPOptAdapter(instance)
    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(mismatch, PortableCapabilityMismatch.UnsupportedVariableKind)
    assert mismatch.kind == kind
    assert mismatch.used_variable_ids == {0}


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

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPySCIPOptAdapter(instance)

    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(mismatch, PortableCapabilityMismatch.UnsupportedOneHotConstraints)
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
