import pytest

from ommx import (
    Constraint,
    DecisionVariable,
    DecisionVariableRole,
    Equality,
    Instance,
    Kind,
    ProvenanceKind,
    Sense,
    SpecialConstraintKind,
)


@pytest.mark.parametrize(
    ("member", "value"),
    [
        (SpecialConstraintKind.Indicator, 1),
        (DecisionVariableRole.Used, 1),
        (Sense.Minimize, 1),
        (Sense.Maximize, 2),
        (Equality.EqualToZero, 1),
        (Equality.LessThanOrEqualToZero, 2),
        (Kind.Binary, 1),
        (Kind.Integer, 2),
        (Kind.Continuous, 3),
        (Kind.SemiInteger, 4),
        (Kind.SemiContinuous, 5),
        (ProvenanceKind.IndicatorConstraint, 1),
    ],
)
def test_eq_int_enums_follow_python_hash_contract(member: object, value: int) -> None:
    enum_keyed: dict[object, str] = {member: "enum"}
    int_keyed: dict[object, str] = {value: "int"}

    assert member == value
    assert hash(member) == hash(value)
    assert enum_keyed[value] == "enum"
    assert int_keyed[member] == "int"


@pytest.mark.parametrize(
    ("alias", "member"),
    [
        (DecisionVariable.BINARY, Kind.Binary),
        (DecisionVariable.INTEGER, Kind.Integer),
        (DecisionVariable.CONTINUOUS, Kind.Continuous),
        (DecisionVariable.SEMI_INTEGER, Kind.SemiInteger),
        (DecisionVariable.SEMI_CONTINUOUS, Kind.SemiContinuous),
        (Constraint.EQUAL_TO_ZERO, Equality.EqualToZero),
        (
            Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            Equality.LessThanOrEqualToZero,
        ),
        (Instance.MINIMIZE, Sense.Minimize),
        (Instance.MAXIMIZE, Sense.Maximize),
    ],
)
def test_compatibility_aliases_are_typed_enum_members(
    alias: object, member: object
) -> None:
    assert type(alias) is type(member)
    assert alias == member


def test_public_model_properties_return_typed_enum_members() -> None:
    variable = DecisionVariable.binary(0)
    constraint = variable <= 1
    instance = Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        constraints={0: constraint},
        sense=Sense.Minimize,
    )
    attached_variable = instance.decision_variables[0]
    attached_constraint = instance.constraints[0]
    solution = instance.evaluate({0: 0})
    sample_set = instance.evaluate_samples([{0: 0}])

    assert type(variable.kind) is Kind
    assert type(constraint.equality) is Equality
    assert type(instance.sense) is Sense
    assert type(attached_variable.kind) is Kind
    assert type(attached_constraint.equality) is Equality
    assert type(solution.sense) is Sense
    assert type(solution.decision_variables[0].kind) is Kind
    assert type(solution.constraints[0].equality) is Equality
    assert type(sample_set.sense) is Sense
    assert type(sample_set.decision_variables[0].kind) is Kind
    assert type(sample_set.constraints[0].equality) is Equality
