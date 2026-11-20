# pyright: reportUnnecessaryTypeIgnoreComment=error

from collections import UserDict

import pytest
from ommx import Samples, State


def test_create_state_from_dict():
    # dict[int, float]
    state = State({1: 0.0, 2: 1.0, 3: 0.0, 4: 1.0, 5: 1.0})
    assert len(state.entries) == 5

    # dict[int, int]
    state = State({1: 0, 2: 1, 3: 0, 4: 1, 5: 1})
    assert len(state.entries) == 5

    # dict[int, float | int]
    state = State({1: 0.0, 2: 1, 3: 0.0, 4: 1, 5: 1})
    assert len(state.entries) == 5


def test_create_state_from_mapping():
    state = State(UserDict({4: 37.5, 0: 2}))
    assert state.entries == {4: 37.5, 0: 2.0}


def test_create_state_from_state():
    original = State({4: 37.5})
    assert State(original).entries == {4: 37.5}


def test_create_state_rejects_invalid_mapping():
    with pytest.raises(TypeError):
        State({(0, 1): 37})  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        State(UserDict({(0, 1): 37}))  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        State({0: None})  # pyright: ignore[reportArgumentType]


def test_state_inputs_reject_pair_iterables():
    pairs = [(0, 37.0)]
    with pytest.raises(TypeError, match=r"dict\(pairs\)"):
        State(pairs)  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        State(pair for pair in pairs)  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        State([(1, 0.0, 1.0)])  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        State((1, 0.0))  # pyright: ignore[reportArgumentType]
    assert State(dict(pairs)).entries == {0: 37.0}


def test_samples_accepts_iterable_of_mappings():
    samples = Samples({0: value} for value in (37.0, 42.0))
    assert samples.get_state(0).entries == {0: 37.0}
    assert samples.get_state(1).entries == {0: 42.0}
    with pytest.raises(TypeError):
        Samples([[(0, 37.0)]])  # pyright: ignore[reportArgumentType]


def test_create_state_from_v1_bytes_roundtrip():
    """Test that State can be serialized and deserialized via from_v1_bytes."""
    state = State({1: 0.0, 2: 1.0, 3: 2.5})
    restored = State.from_v1_bytes(state.to_v1_bytes())
    assert dict(state.entries) == dict(restored.entries)


def test_state_normalizes_negative_zero():
    """-0.0 should be normalized to 0.0 in State entries."""
    state = State({1: -0.0, 2: 1.0})
    assert state.entries[1] == 0.0
    assert str(state.entries[1]) == "0.0"  # Not "-0.0"
