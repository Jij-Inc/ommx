import pytest
from ommx import Instance, DecisionVariable, Rng


def _single_binary_instance() -> Instance:
    x = DecisionVariable.binary(0)
    return Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Instance.MAXIMIZE,
    )


def test_random_samples_basic():
    """Test basic functionality of random_samples"""
    # Create a simple instance
    x = [DecisionVariable.binary(i) for i in range(5)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={0: sum(x) <= 3},  # type: ignore
        sense=Instance.MAXIMIZE,
    )

    # Generate random samples
    rng = Rng()
    samples = instance.random_samples(
        rng,
        num_different_samples=3,
        num_samples=10,
    )

    # Check structure
    assert samples.num_samples() == 10
    # Note: The actual number of unique sample IDs may differ from num_different_samples

    # Check that each state respects variable bounds
    for sample_id in samples.sample_ids():
        state = samples.get_state(sample_id)
        for var_id, value in state.entries.items():
            assert value in [0.0, 1.0], (
                f"Binary variable {var_id} has invalid value {value}"
            )


def test_random_samples_only_used_variables():
    """Test that random_samples only generates values for used variables"""
    # Create instance with some unused variables
    x = [DecisionVariable.binary(i) for i in range(10)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + x[1] + x[2],  # Only use first 3 variables
        constraints={0: x[1] + x[3] <= 1},  # Also use x[3]
        sense=Instance.MAXIMIZE,
    )

    # Generate samples
    rng = Rng()
    samples = instance.random_samples(
        rng,
        num_different_samples=2,
        num_samples=5,
    )

    # Check that only used variables have values
    used_vars = {0, 1, 2, 3}
    assert samples.num_samples() == 5

    for sample_id in samples.sample_ids():
        state = samples.get_state(sample_id)
        assert set(state.entries.keys()) == used_vars


def test_random_samples_with_different_variable_types():
    """Test random_samples with mixed variable types"""
    # Create instance with different variable types
    x_bin = DecisionVariable.binary(0)
    x_int = DecisionVariable.integer(1, lower=5, upper=10)
    x_cont = DecisionVariable.continuous(2, lower=-1.5, upper=2.5)

    instance = Instance.from_components(
        decision_variables=[x_bin, x_int, x_cont],
        objective=x_bin + x_int + x_cont,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    # Generate samples
    rng = Rng()
    samples = instance.random_samples(
        rng,
        num_different_samples=4,
        num_samples=20,
        max_sample_id=50,
    )

    # Check bounds for each variable type
    assert samples.num_samples() == 20

    for sample_id in samples.sample_ids():
        state = samples.get_state(sample_id)

        # Binary variable
        assert state.entries[0] in [0.0, 1.0]

        # Integer variable
        assert 5.0 <= state.entries[1] <= 10.0
        assert state.entries[1] == int(state.entries[1])  # Check it's integer

        # Continuous variable
        assert -1.5 <= state.entries[2] <= 2.5


def test_random_samples_custom_max_sample_id():
    """Test random_samples with custom max_sample_id"""
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        sense=Instance.MAXIMIZE,
    )

    # Generate samples with custom max_sample_id
    rng = Rng()
    samples = instance.random_samples(
        rng,
        num_different_samples=2,
        num_samples=5,
        max_sample_id=100,
    )

    # Check basic structure and sample ID bounds
    assert samples.num_samples() == 5

    # Check that all sample IDs are within bounds
    all_ids = samples.sample_ids()
    assert all(0 <= sample_id <= 100 for sample_id in all_ids)


@pytest.mark.parametrize(
    ("num_different_samples", "num_samples", "max_sample_id", "message"),
    [
        (2, 1, 10, "less than or equal"),
        (0, 1, 10, "must be positive"),
        (1, 2, 0, "sample ID capacity"),
    ],
)
def test_random_samples_invalid_parameters_raise_value_error(
    num_different_samples: int,
    num_samples: int,
    max_sample_id: int,
    message: str,
):
    instance = _single_binary_instance()

    with pytest.raises(ValueError, match=message):
        instance.random_samples(
            Rng(),
            num_different_samples=num_different_samples,
            num_samples=num_samples,
            max_sample_id=max_sample_id,
        )


def test_random_samples_accepts_minimal_nontrivial_partition():
    samples = _single_binary_instance().random_samples(
        Rng(),
        num_different_samples=2,
        num_samples=3,
        max_sample_id=2,
    )

    assert samples.num_samples() == 3


def test_random_samples_accepts_full_u64_sample_id_range():
    samples = _single_binary_instance().random_samples(
        Rng(),
        num_different_samples=1,
        num_samples=1,
        max_sample_id=2**64 - 1,
    )

    assert samples.num_samples() == 1


def test_random_samples_accepts_empty_full_u64_sample_id_range():
    samples = _single_binary_instance().random_samples(
        Rng(),
        num_different_samples=0,
        num_samples=0,
        max_sample_id=2**64 - 1,
    )

    assert samples.num_samples() == 0


def test_random_samples_rejects_sample_id_above_u64():
    with pytest.raises(OverflowError):
        _single_binary_instance().random_samples(
            Rng(),
            num_different_samples=1,
            num_samples=1,
            max_sample_id=2**64,
        )
