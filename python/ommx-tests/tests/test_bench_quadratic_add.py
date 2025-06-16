import pytest
from ommx.v1 import Quadratic, Rng


@pytest.fixture(params=[100, 1000, 10_000])
def quadratic_small_many(request):
    """Create many small quadratic functions with 5 terms each"""
    num_functions = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(num_functions):
        # Create quadratic function with 5 terms using the new random method
        func = Quadratic.random(rng, num_terms=5, max_id=num_functions)
        functions.append(func)
    return functions


@pytest.fixture(params=[100, 1000, 10_000])
def quadratic_large_little(request):
    """Create few large quadratic functions with many terms"""
    num_terms = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(3):  # Only 3 functions
        # Create quadratic function with many terms using the new random method
        func = Quadratic.random(rng, num_terms=num_terms, max_id=3 * num_terms)
        functions.append(func)
    return functions


def sum_quadratic_functions(functions: list[Quadratic]):
    """Sum many quadratic functions"""
    result = Quadratic(columns=[], rows=[], values=[])
    for func in functions:
        result += func
    return result


@pytest.mark.benchmark
def test_sum_quadratic_small_many(benchmark, quadratic_small_many):
    """Benchmark summing many small quadratic functions"""
    benchmark(sum_quadratic_functions, quadratic_small_many)


@pytest.mark.benchmark
def test_sum_quadratic_large_little(benchmark, quadratic_large_little):
    """Benchmark summing few large quadratic functions"""
    benchmark(sum_quadratic_functions, quadratic_large_little)
