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

def test_create_size_mismatch():
    with pytest.raises(TypeError) as e:
        _state = State([(1, 0.0, 1.0)])
    assert str(e.value) == "ommx.v1.State can only be initialized with a `dict[int, float]` or `Iterable[tuple[int, float]]`"

    with pytest.raises(TypeError) as e:
        _state = State((1, 0.0))
    assert str(e.value) == "ommx.v1.State can only be initialized with a `dict[int, float]` or `Iterable[tuple[int, float]]`"

