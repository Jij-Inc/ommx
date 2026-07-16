import pytest

from ommx import (
    AdditionalCapability,
    DecisionVariableRole,
    Equality,
    Kind,
    ProvenanceKind,
    Sense,
)


@pytest.mark.parametrize(
    ("member", "value"),
    [
        (AdditionalCapability.Indicator, 1),
        (DecisionVariableRole.Used, 1),
        (Sense.Minimize, 1),
        (Equality.EqualToZero, 1),
        (Kind.Binary, 1),
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
