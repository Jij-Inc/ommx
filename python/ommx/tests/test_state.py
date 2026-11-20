from collections import UserDict

import pytest
from ommx import State


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


@pytest.mark.parametrize("entries", [{(0, 1): 37}, UserDict({(0, 1): 37}), {0: None}])
def test_create_state_rejects_invalid_mapping(entries):
    with pytest.raises(TypeError):
        State(entries)


def test_create_state_from_generator():
    state = State((key, key + 0.5) for key in (0, 4))
    assert state.entries == {0: 0.5, 4: 4.5}


def test_create_state_from_list():
    state = State([(1, 0.0), (2, 1.0), (3, 0.0), (4, 1.0), (5, 1.0)])
    assert len(state.entries) == 5

    state = State([(1, 0), (2, 1), (3, 0), (4, 1), (5, 1)])
    assert len(state.entries) == 5

    state = State([(1, 0.0), (2, 1.0), (3, 0), (4, 1), (5, 1)])
    assert len(state.entries) == 5


def test_create_state_from_v1_bytes_roundtrip():
    """Test that State can be serialized and deserialized via from_v1_bytes."""
    state = State({1: 0.0, 2: 1.0, 3: 2.5})
    restored = State.from_v1_bytes(state.to_v1_bytes())
    assert dict(state.entries) == dict(restored.entries)


def test_state_normalizes_negative_zero():
    """-0.0 should be normalized to 0.0 in State entries."""
    # From dict
    state = State({1: -0.0, 2: 1.0})
    assert state.entries[1] == 0.0
    assert str(state.entries[1]) == "0.0"  # Not "-0.0"

    # From iterable
    state = State([(1, -0.0), (2, 1.0)])
    assert state.entries[1] == 0.0
    assert str(state.entries[1]) == "0.0"


def test_create_size_mismatch():
    with pytest.raises(TypeError) as e:
        _state = State([(1, 0.0, 1.0)])  # type: ignore[arg-type]
    assert (
        "ommx.State can only be initialized with a `State`, `Mapping[int, float]`, or `Iterable[tuple[int, float]]`"
        in str(e.value)
    )

    with pytest.raises(TypeError) as e:
        _state = State((1, 0.0))  # type: ignore[arg-type]
    assert (
        "ommx.State can only be initialized with a `State`, `Mapping[int, float]`, or `Iterable[tuple[int, float]]`"
        in str(e.value)
    )
