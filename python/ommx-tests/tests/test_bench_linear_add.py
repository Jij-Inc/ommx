import pytest
from ommx.v1 import Linear, Rng


@pytest.fixture(params=[100, 1000, 10_000])
def linear_small_many(request):
    """Create many small linear functions with 3 terms each"""
    num_functions = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(num_functions):
        # Create linear function with 3 terms using the new random method
        func = Linear.random(rng, num_terms=3, max_id=num_functions)
        functions.append(func)
    return functions


@pytest.fixture(params=[100, 1000, 10_000])
def linear_large_little(request):
    """Create few large linear functions with many terms"""
    num_terms = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(3):  # Only 3 functions
        # Create linear function with many terms using the new random method
        func = Linear.random(rng, num_terms=num_terms, max_id=3 * num_terms)
        functions.append(func)
    return functions


def sum_linear_functions(functions: list[Linear]):
    """Sum many linear functions"""
    result = Linear(terms={}, constant=0)
    for func in functions:
        result += func
    return result


@pytest.mark.benchmark
def test_sum_linear_small_many(benchmark, linear_small_many):
    """Benchmark summing many small linear functions"""
    benchmark(sum_linear_functions, linear_small_many)


@pytest.mark.benchmark
def test_sum_linear_large_little(benchmark, linear_large_little):
    """Benchmark summing few large linear functions"""
    benchmark(sum_linear_functions, linear_large_little)
