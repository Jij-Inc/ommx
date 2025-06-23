from ommx import _ommx_rust
import pytest


def test_create_samples_from_single_state():
    """Test creating Samples from a single state (dict[int, float])"""
    # Single state as dict[int, float] - should become sample ID 0
    samples = _ommx_rust.Samples({1: 0.0, 2: 1.0, 3: 0.5})
    assert samples.num_samples() == 1
    assert 0 in samples.sample_ids()


def test_create_samples_from_dict_of_states():
    """Test creating Samples from dict[int, dict[int, float]]"""
    # Multiple states with explicit sample IDs
    samples = _ommx_rust.Samples(
        {0: {1: 0.0, 2: 1.0}, 1: {1: 1.0, 2: 0.0}, 5: {1: 0.5, 2: 0.5}}
    )
    assert samples.num_samples() == 3
    sample_ids = samples.sample_ids()
    assert 0 in sample_ids
    assert 1 in sample_ids
    assert 5 in sample_ids


def test_create_samples_from_list():
    """Test creating Samples from list[dict[int, float]]"""
    # List of states - should enumerate with sample IDs 0, 1, 2
    samples = _ommx_rust.Samples([{1: 0.0, 2: 1.0}, {1: 1.0, 2: 0.0}, {1: 0.5, 2: 0.5}])
    assert samples.num_samples() == 3
    sample_ids = samples.sample_ids()
    assert 0 in sample_ids
    assert 1 in sample_ids
    assert 2 in sample_ids


def test_samples_serialization():
    """Test Samples serialization and deserialization"""
    original = _ommx_rust.Samples({0: {1: 0.0, 2: 1.0}, 1: {1: 1.0, 2: 0.0}})

    # Serialize to bytes
    data = original.to_bytes()
    assert isinstance(data, bytes)

    # Deserialize from bytes
    restored = _ommx_rust.Samples.from_bytes(data)
    assert restored.num_samples() == original.num_samples()
    assert restored.sample_ids() == original.sample_ids()


def test_invalid_samples_creation():
    """Test error cases for Samples creation"""
    # Invalid type
    with pytest.raises(TypeError) as e:
        _ommx_rust.Samples("invalid")
    assert "entries must be a State, dict[int, State], or iterable[State]" in str(e.value)

    # Invalid dictionary values
    with pytest.raises(TypeError) as e:
        _ommx_rust.Samples({0: "not_a_state"})
    assert "Dictionary values must be State objects or dict[int, float]" in str(e.value)

    # Invalid iterable items
    with pytest.raises(TypeError) as e:
        _ommx_rust.Samples(["not_a_state"])
    assert "Iterable items must be State objects or dict[int, float]" in str(e.value)


def test_empty_samples():
    """Test creating empty samples"""
    # Empty dict should create no samples
    samples = _ommx_rust.Samples({})
    assert samples.num_samples() == 0
    assert len(samples.sample_ids()) == 0

    # Empty list should create no samples
    samples = _ommx_rust.Samples([])
    assert samples.num_samples() == 0
    assert len(samples.sample_ids()) == 0


def test_dict_with_string_keys():
    """Test creating Samples from dict with string keys (should fail)"""
    # Dictionary with string keys should be rejected
    with pytest.raises(TypeError) as e:
        _ommx_rust.Samples({
            "sample1": {1: 0.0, 2: 1.0},
            "sample2": {1: 1.0, 2: 0.0}
        })
    assert "entries must be a State, dict[int, State], or iterable[State]" in str(e.value)
    
    # Mixed string and int keys should also be rejected
    with pytest.raises(TypeError) as e:
        _ommx_rust.Samples({
            0: {1: 0.0, 2: 1.0},
            "sample": {1: 1.0, 2: 0.0}
        })
    assert "entries must be a State, dict[int, State], or iterable[State]" in str(e.value)


def test_create_samples_from_iterable():
    """Test creating Samples from various iterable types (not just lists)"""
    # Tuple of states - should enumerate with sample IDs 0, 1, 2
    samples = _ommx_rust.Samples((
        {1: 0.0, 2: 1.0},
        {1: 1.0, 2: 0.0},
        {1: 0.5, 2: 0.5}
    ))
    assert samples.num_samples() == 3
    sample_ids = samples.sample_ids()
    assert 0 in sample_ids
    assert 1 in sample_ids
    assert 2 in sample_ids
    
    # Empty tuple should create no samples
    samples = _ommx_rust.Samples(())
    assert samples.num_samples() == 0
    assert len(samples.sample_ids()) == 0
