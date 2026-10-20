import copy
import math
from collections import UserDict
from types import MappingProxyType
from typing import Any

import ommx
import pytest
from ommx import DecisionVariable, Instance, NamedState
from ommx.v1 import State


def make_instance(*variables: DecisionVariable) -> Instance:
    return Instance.from_components(
        decision_variables=variables,
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )


def test_named_state_is_exported():
    from ommx.v1 import NamedState as V1NamedState

    assert "NamedState" in ommx.__all__
    assert ommx.NamedState is NamedState is V1NamedState


def test_scalar_multidimensional_and_partial_values():
    omitted = DecisionVariable.binary(7, name="omitted")
    scalar = DecisionVariable.continuous(31, name="scalar")
    indexed = DecisionVariable.continuous(107, name="x", subscripts=[-2, 3])
    instance = make_instance(omitted, scalar, indexed)
    values: NamedState = {"scalar": {(): 2.5}, "x": {(-2, 3): -1.0}}

    state = instance.state_from_named_values(values)

    assert isinstance(state, State)
    assert state.entries == {scalar.id: 2.5, indexed.id: -1.0}
    assert omitted.id not in state.entries
    assert values == {"scalar": {(): 2.5}, "x": {(-2, 3): -1.0}}
    assert instance.state_from_named_values({}).entries == {}
    assert instance.state_from_named_values({"x": {}}).entries == {}


def test_reuses_solution_values_with_different_target_ids():
    x = DecisionVariable.binary(0, name="x", subscripts=[0])
    y = DecisionVariable.binary(1, name="x", subscripts=[1])
    source = make_instance(x, y)
    values = source.evaluate({x.id: 1.0, y.id: 0.0}).extract_all_decision_variables()
    assert source.state_from_named_values(values).entries == {x.id: 1.0, y.id: 0.0}
    target_x = DecisionVariable.binary(900, name="x", subscripts=[0])
    target_y = DecisionVariable.binary(41, name="x", subscripts=[1])
    target = make_instance(target_x, target_y)

    assert target.state_from_named_values(values).entries == {
        target_x.id: 1.0,
        target_y.id: 0.0,
    }


@pytest.mark.parametrize("mapping_type", [UserDict, MappingProxyType])
def test_accepts_nested_mappings(mapping_type: type):
    x = DecisionVariable.binary(31, name="x", subscripts=[0])
    state = make_instance(x).state_from_named_values(
        mapping_type({"x": mapping_type({(0,): 1.0})})
    )
    assert state.entries == {x.id: 1.0}


@pytest.mark.parametrize(
    "values, name, subscripts",
    [
        ({"missing": {(0,): 1.0}}, "missing", "[0]"),
        ({"x": {(1,): 1.0}}, "x", "[1]"),
        ({"x": {(): 1.0}}, "x", "[]"),
    ],
)
def test_unknown_key_reports_name_and_subscripts(values, name, subscripts):
    instance = make_instance(DecisionVariable.binary(31, name="x", subscripts=[0]))
    before = instance.to_bytes()
    original = copy.deepcopy(values)

    with pytest.raises(KeyError) as error:
        instance.state_from_named_values(values)

    assert name in str(error.value)
    assert subscripts in str(error.value)
    assert values == original
    assert instance.to_bytes() == before


def test_ambiguous_key_rejected_even_when_parameters_differ():
    a = DecisionVariable.binary(31, name="x", subscripts=[0], parameters={"group": "a"})
    b = DecisionVariable.binary(
        107, name="x", subscripts=[0], parameters={"group": "b"}
    )
    unique = DecisionVariable.binary(41, name="x", subscripts=[1])
    instance = make_instance(a, b, unique)
    values: NamedState = {"x": {(0,): 1.0}}
    before = instance.to_bytes()

    with pytest.raises(ValueError, match="use an ID-based state") as error:
        instance.state_from_named_values(values)

    assert "name 'x'" in str(error.value)
    assert "subscripts [0]" in str(error.value)
    assert str(a.id) in str(error.value)
    assert str(b.id) in str(error.value)
    assert instance.to_bytes() == before
    assert values == {"x": {(0,): 1.0}}
    assert instance.state_from_named_values({"x": {(1,): 0.0}}).entries == {
        unique.id: 0.0
    }


def test_empty_string_name_is_distinct_from_an_absent_name():
    unnamed = DecisionVariable.binary(0)
    named = DecisionVariable.binary(31, name="")
    assert unnamed.name == named.name == ""
    assert make_instance(unnamed, named).state_from_named_values(
        {"": {(): 1.0}}
    ).entries == {named.id: 1.0}
    with pytest.raises(KeyError):
        make_instance(unnamed).state_from_named_values({"": {(): 1.0}})


@pytest.mark.parametrize(
    "values",
    [
        [],
        {0: {(): 1.0}},
        {"x": []},
        {"x": {0: 1.0}},
        {"x": {("0",): 1.0}},
        {"x": {(): "bad"}},
    ],
)
def test_rejects_invalid_input_shapes(values: Any):
    with pytest.raises(TypeError):
        make_instance(DecisionVariable.binary(31, name="x")).state_from_named_values(
            values
        )


def test_rejects_subscripts_outside_i64():
    with pytest.raises(OverflowError):
        make_instance().state_from_named_values({"x": {(1 << 63,): 1.0}})


def test_preserves_mapping_callback_error():
    class BrokenMapping(UserDict):
        def items(self):
            raise RuntimeError("mapping failed")

    with pytest.raises(RuntimeError, match="mapping failed"):
        make_instance().state_from_named_values(BrokenMapping())


@pytest.mark.parametrize("value", [-0.0, 2.5, math.inf, -math.inf, math.nan])
def test_values_follow_state_constructor_semantics(value: float):
    x = DecisionVariable.binary(31, name="x")
    actual = make_instance(x).state_from_named_values({"x": {(): value}}).entries[x.id]
    expected = State({x.id: value}).entries[x.id]
    if math.isnan(expected):
        assert math.isnan(actual)
    else:
        assert actual == expected
        assert math.copysign(1.0, actual) == (
            1.0 if value == 0.0 else math.copysign(1.0, expected)
        )
