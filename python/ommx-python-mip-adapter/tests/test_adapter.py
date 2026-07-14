import pytest
from ommx import (
    DecisionVariable,
    DegreeLimit,
    Equality,
    IndicatorConstraint,
    Instance,
    Kind,
    OneHotConstraint,
    PortableCapabilityMismatch,
    Sense,
    Sos1Constraint,
)
from ommx.adapter import AdapterCompatibilityError
from ommx_python_mip_adapter import OMMXPythonMIPAdapter


def test_declares_native_linear_mip_capability_profile():
    capabilities = OMMXPythonMIPAdapter.CAPABILITIES
    assert capabilities is not None
    [profile] = capabilities.profiles
    assert profile.name == "python-mip-linear-mip"
    assert profile.variable_kinds == {Kind.Binary, Kind.Integer, Kind.Continuous}
    assert profile.objective_degree == DegreeLimit.at_most(1)
    assert profile.regular_constraints == {
        Equality.EqualToZero: DegreeLimit.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(1),
    }
    assert profile.indicator_constraints == {}
    assert not profile.supports_one_hot
    assert not profile.supports_sos1
    assert profile.senses == {Sense.Minimize, Sense.Maximize}


@pytest.mark.parametrize("sense", [Sense.Minimize, Sense.Maximize])
def test_capability_profile_accepts_complete_linear_mip_boundary(sense):
    x = DecisionVariable.binary(0)
    y = DecisionVariable.integer(1)
    z = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        decision_variables=[x, y, z],
        objective=x + y + z,
        constraints={0: x + y == 1, 1: z <= 1},
        sense=sense,
    )

    report = OMMXPythonMIPAdapter.check_compatibility(instance)
    assert report.compatible
    assert report.portable_report.matching_profiles == ["python-mip-linear-mip"]


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x (variable ID should match)
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert isinstance(
        e.value.report.portable_report.profiles[0].mismatches[0],
        PortableCapabilityMismatch.ObjectiveDegreeExceeded,
    )


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0.0,
        constraints={0: 2.3 * x * x == 0},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert isinstance(
        e.value.report.portable_report.profiles[0].mismatches[0],
        PortableCapabilityMismatch.RegularConstraintDegreeExceeded,
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

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPythonMIPAdapter(instance)
    mismatch = e.value.report.portable_report.profiles[0].mismatches[0]
    assert isinstance(mismatch, PortableCapabilityMismatch.UnsupportedVariableKind)
    assert mismatch.kind == kind
    assert mismatch.used_variable_ids == {0}


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

    with pytest.raises(AdapterCompatibilityError) as e:
        OMMXPythonMIPAdapter(instance)

    mismatch_types = {
        type(mismatch)
        for mismatch in e.value.report.portable_report.profiles[0].mismatches
    }
    assert PortableCapabilityMismatch.UnsupportedIndicatorConstraints in mismatch_types
    assert PortableCapabilityMismatch.UnsupportedOneHotConstraints in mismatch_types
    assert PortableCapabilityMismatch.UnsupportedSos1Constraints in mismatch_types
    assert instance.to_v2_bytes() == before
