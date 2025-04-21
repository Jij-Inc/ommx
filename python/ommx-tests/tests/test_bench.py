import pytest
from statistics import median


@pytest.mark.parametrize("input_size", [10, 100, 1000])
def test_to_qubo(benchmark, input_size):
    def to_qubo(input_size):
        # Simulate a function that converts to QUBO
        return median([i for i in range(input_size)])

    result = benchmark(to_qubo, input_size)
    assert isinstance(result, float)
