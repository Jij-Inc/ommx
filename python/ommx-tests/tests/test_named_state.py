import copy
import math
from collections import UserDict
from collections.abc import Mapping
from types import MappingProxyType
from typing import Any

import ommx
import pytest
from ommx import Instance, NamedState, State


def test_named_state_is_exported():
    assert "NamedState" in ommx.__all__
    assert ommx.NamedState is NamedState
    assert NamedState == Mapping[str, Mapping[tuple[int, ...], float]]


def test_scalar_multidimensional_and_partial_values():
    instance = Instance.minimize()
    omitted = instance.new_binary("omitted")
    scalar = instance.new_continuous("scalar")
    indexed = instance.new_continuous("x", subscripts=[-2, 3])
    values: NamedState = {"scalar": {(): 2.5}, "x": {(-2, 3): -1.0}}

    state = instance.state_from_named_values(values)

    assert isinstance(state, State)
    assert state.entries == {scalar.id: 2.5, indexed.id: -1.0}
    assert omitted.id not in state.entries
    assert values == {"scalar": {(): 2.5}, "x": {(-2, 3): -1.0}}
    assert instance.state_from_named_values({}).entries == {}
    assert instance.state_from_named_values({"x": {}}).entries == {}


def test_reuses_solution_values_with_different_target_ids():
    source = Instance.minimize()
    x = source.new_binary("x", subscripts=[0])
    y = source.new_binary("x", subscripts=[1])
    values = source.evaluate({x.id: 1.0, y.id: 0.0}).extract_all_decision_variables()
    assert source.state_from_named_values(values).entries == {x.id: 1.0, y.id: 0.0}

    target = Instance.minimize()
    target.new_binary()  # Shift IDs and reverse the order of creation.
    target_y = target.new_binary("x", subscripts=[1])
    target_x = target.new_binary("x", subscripts=[0])

    state = target.state_from_named_values(values)

    assert state.entries == {target_x.id: 1.0, target_y.id: 0.0}


@pytest.mark.parametrize("mapping_type", [UserDict, MappingProxyType])
def test_accepts_nested_mappings(mapping_type: type):
    instance = Instance.minimize()
    x = instance.new_binary("x", subscripts=[0])

    state = instance.state_from_named_values(
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
    instance = Instance.minimize()
    instance.new_binary("x", subscripts=[0])
    before = instance.to_v2_bytes()
    original = copy.deepcopy(values)

    with pytest.raises(KeyError) as error:
        instance.state_from_named_values(values)

    assert name in str(error.value)
    assert subscripts in str(error.value)
    assert values == original
    assert instance.to_v2_bytes() == before


def test_ambiguous_key_rejected_even_when_parameters_differ():
    instance = Instance.minimize()
    a = instance.new_binary("x", subscripts=[0], parameters={"group": "a"})
    b = instance.new_binary("x", subscripts=[0], parameters={"group": "b"})
    unique = instance.new_binary("x", subscripts=[1])
    values: NamedState = {"x": {(0,): 1.0}}
    before = instance.to_v2_bytes()

    with pytest.raises(ValueError, match="use an ID-based state") as error:
        instance.state_from_named_values(values)

    assert 'name "x"' in str(error.value)
    assert "subscripts [0]" in str(error.value)
    assert str(a.id) in str(error.value)
    assert str(b.id) in str(error.value)
    assert instance.to_v2_bytes() == before
    assert values == {"x": {(0,): 1.0}}
    assert instance.state_from_named_values({"x": {(1,): 0.0}}).entries == {
        unique.id: 0.0
    }


def test_reads_current_labels_on_each_call():
    instance = Instance.minimize()
    x = instance.new_binary("x", subscripts=[0])
    assert instance.state_from_named_values({"x": {(0,): 1.0}}).entries == {x.id: 1.0}

    x.set_name("y")
    x.set_subscripts([1])

    with pytest.raises(KeyError):
        instance.state_from_named_values({"x": {(0,): 1.0}})
    assert instance.state_from_named_values({"y": {(1,): 1.0}}).entries == {x.id: 1.0}


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
    instance = Instance.minimize()
    instance.new_binary("x")

    with pytest.raises(TypeError):
        instance.state_from_named_values(values)


def test_preserves_mapping_callback_error():
    class BrokenMapping(UserDict):
        def items(self):
            raise RuntimeError("mapping failed")

    with pytest.raises(RuntimeError, match="mapping failed"):
        Instance.minimize().state_from_named_values(BrokenMapping())


@pytest.mark.parametrize("value", [-0.0, 2.5, math.inf, -math.inf, math.nan])
def test_values_follow_state_constructor_semantics(value: float):
    instance = Instance.minimize()
    x = instance.new_binary("x")

    actual = instance.state_from_named_values({"x": {(): value}}).entries[x.id]
    expected = State({x.id: value}).entries[x.id]

    if math.isnan(expected):
        assert math.isnan(actual)
    else:
        assert actual == expected
        assert math.copysign(1.0, actual) == math.copysign(1.0, expected)
