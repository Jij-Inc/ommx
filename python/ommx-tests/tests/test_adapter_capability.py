from __future__ import annotations

from typing import Any, cast

import pytest

from ommx import (
    AdapterCapabilities,
    CapabilityProfile,
    DecisionVariable,
    DegreeLimit,
    Equality,
    IndicatorConstraint,
    Instance,
    Kind,
    OneHotConstraint,
    PortableCapabilityMismatch,
    PortableCompatibilityReport,
    Sense,
    Sos1Constraint,
    SpecialConstraintKind,
)
from ommx.adapter import (
    AdapterCompatibilityError,
    AdapterPreconditionViolation,
    ConstraintRef,
    SolverAdapter,
)


def profile(
    name: str,
    *,
    variable_kinds: set[Kind],
    objective_degree: DegreeLimit,
    senses: set[Sense] | None = None,
    regular_constraints: dict[Equality, DegreeLimit] | None = None,
    indicator_constraints: dict[Equality, DegreeLimit] | None = None,
    supports_one_hot: bool = False,
    supports_sos1: bool = False,
) -> CapabilityProfile:
    return CapabilityProfile(
        name=name,
        variable_kinds=variable_kinds,
        objective_degree=objective_degree,
        senses=senses or {Sense.Minimize},
        regular_constraints=regular_constraints,
        indicator_constraints=indicator_constraints,
        supports_one_hot=supports_one_hot,
        supports_sos1=supports_sos1,
    )


def instance_with_objective(variable: DecisionVariable, objective: Any) -> Instance:
    return Instance.from_components(
        sense=Sense.Minimize,
        objective=objective,
        decision_variables=[variable],
        constraints={},
    )


def binary_linear_capabilities(
    *, supports_one_hot: bool = False
) -> AdapterCapabilities:
    return AdapterCapabilities(
        [
            profile(
                "binary-linear",
                variable_kinds={Kind.Binary},
                objective_degree=DegreeLimit.at_most(1),
                supports_one_hot=supports_one_hot,
            )
        ]
    )


def test_degree_limit_and_declaration_validation() -> None:
    linear = DegreeLimit.at_most(1)
    assert linear.maximum == 1
    assert linear.allows(0)
    assert linear.allows(1)
    assert not linear.allows(2)
    assert DegreeLimit.any().maximum is None
    assert DegreeLimit.any().allows(10_000)

    with pytest.raises(ValueError, match="name must not be empty"):
        profile(
            "",
            variable_kinds={Kind.Binary},
            objective_degree=DegreeLimit.any(),
        )
    with pytest.raises(ValueError, match="at least one capability profile"):
        AdapterCapabilities([])

    duplicate = profile(
        "duplicate",
        variable_kinds={Kind.Binary},
        objective_degree=DegreeLimit.any(),
    )
    with pytest.raises(ValueError, match="Duplicate capability profile"):
        AdapterCapabilities([duplicate, duplicate])

    assert ConstraintRef("regular", 1) != ConstraintRef("indicator", 1)


def test_requirements_cover_every_active_solver_input_family() -> None:
    x = DecisionVariable.binary(1)
    y = DecisionVariable.continuous(2)
    z = DecisionVariable.binary(3)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x * y,
        decision_variables=[x, y, z],
        constraints={10: y <= 2},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x,
                function=y - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={30: OneHotConstraint(variables=[x, z])},
        sos1_constraints={40: Sos1Constraint(variables=[y, z])},
    )

    requirements = instance.solver_requirements()
    assert requirements.sense == Sense.Maximize
    assert requirements.objective_degree == 2
    assert requirements.used_variables_by_kind == {
        Kind.Binary: {1, 3},
        Kind.Continuous: {2},
    }
    assert requirements.used_variable_ids == {1, 2, 3}
    assert (
        requirements.regular_constraints[10].relation == Equality.LessThanOrEqualToZero
    )
    assert requirements.regular_constraints[10].degree == 1
    assert (
        requirements.indicator_constraints[20].relation
        == Equality.LessThanOrEqualToZero
    )
    assert requirements.indicator_constraints[20].degree == 1
    assert requirements.one_hot_constraint_ids == {30}
    assert requirements.sos1_constraint_ids == {40}


def test_special_constraint_lowering_uses_direct_kind_selection() -> None:
    x = DecisionVariable.continuous(1, lower=0, upper=5)
    y = DecisionVariable.binary(2)
    z = DecisionVariable.binary(3)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x, y, z],
        constraints={},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=y,
                function=x - 1,
                equality=Equality.LessThanOrEqualToZero,
            )
        },
        one_hot_constraints={30: OneHotConstraint(variables=[y, z])},
        sos1_constraints={40: Sos1Constraint(variables=[x, z])},
    )

    assert instance.active_special_constraint_kinds == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    }
    assert instance.lower_special_constraints(set()) == set()

    assert instance.lower_special_constraints({SpecialConstraintKind.OneHot}) == {
        SpecialConstraintKind.OneHot
    }
    assert instance.active_special_constraint_kinds == {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.Sos1,
    }
    assert instance.lower_special_constraints({SpecialConstraintKind.OneHot}) == set()


def test_complete_profiles_do_not_cross_combine_into_miqp_support() -> None:
    relations = {
        Equality.EqualToZero: DegreeLimit.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(1),
    }
    capabilities = AdapterCapabilities(
        [
            profile(
                "milp",
                variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_degree=DegreeLimit.at_most(1),
                regular_constraints=relations,
            ),
            profile(
                "continuous-qp",
                variable_kinds={Kind.Continuous},
                objective_degree=DegreeLimit.at_most(2),
                regular_constraints=relations,
            ),
        ]
    )

    continuous = DecisionVariable.continuous(1)
    continuous_qp = capabilities.check_compatibility(
        instance_with_objective(
            continuous, continuous * continuous
        ).solver_requirements()
    )
    assert continuous_qp.compatible
    assert continuous_qp.matching_profiles == ["continuous-qp"]

    integer = DecisionVariable.integer(1)
    milp = capabilities.check_compatibility(
        instance_with_objective(integer, integer).solver_requirements()
    )
    assert milp.compatible
    assert milp.matching_profiles == ["milp"]

    miqp = capabilities.check_compatibility(
        instance_with_objective(integer, integer * integer).solver_requirements()
    )
    assert not miqp.compatible
    assert isinstance(
        miqp.profiles[0].mismatches[0],
        PortableCapabilityMismatch.ObjectiveDegreeExceeded,
    )
    assert isinstance(
        miqp.profiles[1].mismatches[0],
        PortableCapabilityMismatch.UnsupportedVariableKind,
    )


def test_mismatch_variants_preserve_all_structured_payloads() -> None:
    x = DecisionVariable.binary(1)
    y = DecisionVariable.continuous(2)
    instance = Instance.from_components(
        sense=Sense.Maximize,
        objective=x * y,
        decision_variables=[x, y],
        constraints={10: y * y == 0, 11: y <= 0},
        indicator_constraints={
            20: IndicatorConstraint(
                indicator_variable=x,
                function=y * y,
                equality=Equality.EqualToZero,
            ),
            21: IndicatorConstraint(
                indicator_variable=x,
                function=y,
                equality=Equality.LessThanOrEqualToZero,
            ),
        },
        one_hot_constraints={30: OneHotConstraint(variables=[x])},
        sos1_constraints={40: Sos1Constraint(variables=[y])},
    )
    limited = profile(
        "limited",
        variable_kinds={Kind.Binary},
        objective_degree=DegreeLimit.at_most(1),
        senses={Sense.Minimize},
        regular_constraints={Equality.EqualToZero: DegreeLimit.at_most(1)},
        indicator_constraints={Equality.EqualToZero: DegreeLimit.at_most(1)},
    )
    mismatches = (
        AdapterCapabilities([limited])
        .check_compatibility(instance.solver_requirements())
        .profiles[0]
        .mismatches
    )

    unsupported_kind = mismatches[0]
    assert isinstance(
        unsupported_kind, PortableCapabilityMismatch.UnsupportedVariableKind
    )
    assert unsupported_kind.kind == Kind.Continuous
    assert unsupported_kind.used_variable_ids == {2}
    assert unsupported_kind.supported_kinds == {Kind.Binary}

    objective = mismatches[1]
    assert isinstance(objective, PortableCapabilityMismatch.ObjectiveDegreeExceeded)
    assert objective.actual_degree == 2
    assert objective.limit == DegreeLimit.at_most(1)

    regular_degree = mismatches[2]
    assert isinstance(
        regular_degree, PortableCapabilityMismatch.RegularConstraintDegreeExceeded
    )
    assert regular_degree.relation == Equality.EqualToZero
    assert regular_degree.actual_degrees == {10: 2}
    assert regular_degree.limit == DegreeLimit.at_most(1)

    regular_relation = mismatches[3]
    assert isinstance(
        regular_relation,
        PortableCapabilityMismatch.UnsupportedRegularConstraintRelation,
    )
    assert regular_relation.relation == Equality.LessThanOrEqualToZero
    assert regular_relation.constraint_ids == {11}
    assert regular_relation.supported_relations == {Equality.EqualToZero}

    indicator_degree = mismatches[4]
    assert isinstance(
        indicator_degree, PortableCapabilityMismatch.IndicatorBodyDegreeExceeded
    )
    assert indicator_degree.actual_degrees == {20: 2}

    indicator_relation = mismatches[5]
    assert isinstance(
        indicator_relation,
        PortableCapabilityMismatch.UnsupportedIndicatorConstraintRelation,
    )
    assert indicator_relation.constraint_ids == {21}

    assert isinstance(
        mismatches[6], PortableCapabilityMismatch.UnsupportedOneHotConstraints
    )
    assert mismatches[6].constraint_ids == {30}
    assert isinstance(
        mismatches[7], PortableCapabilityMismatch.UnsupportedSos1Constraints
    )
    assert mismatches[7].constraint_ids == {40}

    unsupported_sense = mismatches[8]
    assert isinstance(unsupported_sense, PortableCapabilityMismatch.UnsupportedSense)
    assert unsupported_sense.sense == Sense.Maximize
    assert unsupported_sense.supported_senses == {Sense.Minimize}

    no_indicator_support = profile(
        "no-indicator-support",
        variable_kinds={Kind.Binary, Kind.Continuous},
        objective_degree=DegreeLimit.any(),
        senses={Sense.Minimize, Sense.Maximize},
        regular_constraints={
            Equality.EqualToZero: DegreeLimit.any(),
            Equality.LessThanOrEqualToZero: DegreeLimit.any(),
        },
        supports_one_hot=True,
        supports_sos1=True,
    )
    unsupported_indicator = next(
        mismatch
        for mismatch in AdapterCapabilities([no_indicator_support])
        .check_compatibility(instance.solver_requirements())
        .profiles[0]
        .mismatches
        if isinstance(
            mismatch, PortableCapabilityMismatch.UnsupportedIndicatorConstraints
        )
    )
    assert unsupported_indicator.constraint_ids == {20, 21}


def test_solver_adapter_combines_preconditions_and_preserves_the_caller() -> None:
    x = DecisionVariable.binary(1)
    instance = instance_with_objective(x, x)
    before = instance.to_v2_bytes()
    violation = AdapterPreconditionViolation(
        condition="backend_limit",
        description="backend accepts no variables in this test",
        variable_ids=frozenset({1}),
        actual=1,
        limit=0,
    )

    class Adapter(SolverAdapter):
        CAPABILITIES = binary_linear_capabilities()
        calls = 0

        @classmethod
        def _check_preconditions(cls, ommx_instance, portable_report):
            cls.calls += 1
            assert portable_report.compatible
            assert ommx_instance.to_v2_bytes() == before
            return (violation,)

    report = Adapter.check_compatibility(instance)
    assert not report.compatible
    assert report.portable_report.compatible
    assert report.preconditions_checked
    assert report.precondition_violations == (violation,)
    assert report.adapter.endswith(".Adapter")
    assert Adapter.calls == 1
    assert instance.to_v2_bytes() == before

    with pytest.raises(AdapterCompatibilityError) as exc_info:
        Adapter.require_compatible(instance)
    assert exc_info.value.report.precondition_violations == (violation,)


def test_portable_failure_skips_preconditions_and_missing_declaration_is_error() -> (
    None
):
    y = DecisionVariable.continuous(1)
    incompatible = instance_with_objective(y, y * y)

    class Adapter(SolverAdapter):
        CAPABILITIES = binary_linear_capabilities()
        calls = 0

        @classmethod
        def _check_preconditions(cls, ommx_instance, portable_report):
            cls.calls += 1
            return ()

    report = Adapter.check_compatibility(incompatible)
    assert not report.compatible
    assert not report.preconditions_checked
    assert report.precondition_violations == ()
    assert Adapter.calls == 0

    class MissingDeclaration(SolverAdapter):
        pass

    with pytest.raises(TypeError, match="must declare CAPABILITIES"):
        MissingDeclaration.check_compatibility(incompatible)


def test_precondition_hook_is_isolated_and_validated() -> None:
    x = DecisionVariable.binary(1)
    instance = Instance.from_components(
        sense=Sense.Minimize,
        objective=x,
        decision_variables=[x],
        constraints={},
        one_hot_constraints={30: OneHotConstraint(variables=[x])},
    )
    before = instance.to_v2_bytes()

    class MutatingHook(SolverAdapter):
        CAPABILITIES = binary_linear_capabilities(supports_one_hot=True)

        @classmethod
        def _check_preconditions(
            cls,
            ommx_instance: Instance,
            portable_report: PortableCompatibilityReport,
        ) -> tuple[AdapterPreconditionViolation, ...]:
            ommx_instance.lower_special_constraints({SpecialConstraintKind.OneHot})
            return ()

    assert MutatingHook.check_compatibility(instance).compatible
    assert instance.to_v2_bytes() == before
    assert instance.solver_requirements().one_hot_constraint_ids == {30}

    class InvalidHook(SolverAdapter):
        CAPABILITIES = binary_linear_capabilities(supports_one_hot=True)

        @classmethod
        def _check_preconditions(
            cls,
            ommx_instance: Instance,
            portable_report: PortableCompatibilityReport,
        ) -> tuple[AdapterPreconditionViolation, ...]:
            return cast(tuple[AdapterPreconditionViolation, ...], ("not a violation",))

    with pytest.raises(TypeError, match="AdapterPreconditionViolation"):
        InvalidHook.check_compatibility(instance)
