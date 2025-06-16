import pytest
from ommx.v1 import Polynomial, Rng


@pytest.fixture(params=[100, 1000, 10_000])
def polynomial_small_many(request):
    """Create many small polynomial functions with 5 terms each"""
    num_functions = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(num_functions):
        # Create polynomial function with 5 terms using the new random method
        func = Polynomial.random(rng, num_terms=5, max_degree=3, max_id=num_functions)
        functions.append(func)
    return functions


@pytest.fixture(params=[100, 1000, 10_000])
def polynomial_large_little(request):
    """Create few large polynomial functions with many terms"""
    num_terms = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(3):  # Only 3 functions
        # Create polynomial function with many terms using the new random method
        func = Polynomial.random(rng, num_terms=num_terms, max_degree=3, max_id=3 * num_terms)
        functions.append(func)
    return functions


def sum_polynomial_functions(functions: list[Polynomial]):
    """Sum many polynomial functions"""
    result = Polynomial(terms={})
    for func in functions:
        result += func
    return result


@pytest.mark.benchmark
def test_sum_polynomial_small_many(benchmark, polynomial_small_many):
    """Benchmark summing many small polynomial functions"""
    benchmark(sum_polynomial_functions, polynomial_small_many)


@pytest.mark.benchmark
def test_sum_polynomial_large_little(benchmark, polynomial_large_little):
    """Benchmark summing few large polynomial functions"""
    benchmark(sum_polynomial_functions, polynomial_large_little)