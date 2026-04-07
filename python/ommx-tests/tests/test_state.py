from ommx.v1 import State
import pytest


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


def test_create_state_from_list():
    state = State([(1, 0.0), (2, 1.0), (3, 0.0), (4, 1.0), (5, 1.0)])
    assert len(state.entries) == 5

    state = State([(1, 0), (2, 1), (3, 0), (4, 1), (5, 1)])
    assert len(state.entries) == 5

    state = State([(1, 0.0), (2, 1.0), (3, 0), (4, 1), (5, 1)])
    assert len(state.entries) == 5


def test_create_state_from_bytes_roundtrip():
    """Test that State can be serialized and deserialized via from_bytes."""
    state = State({1: 0.0, 2: 1.0, 3: 2.5})
    restored = State.from_bytes(state.to_bytes())
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
        str(e.value)
        == "ommx.v1.State can only be initialized with a `State`, `Mapping[int, float]`, or `Iterable[tuple[int, float]]`"
    )

    with pytest.raises(TypeError) as e:
        _state = State((1, 0.0))  # type: ignore[arg-type]
    assert (
        str(e.value)
        == "ommx.v1.State can only be initialized with a `State`, `Mapping[int, float]`, or `Iterable[tuple[int, float]]`"
    )
