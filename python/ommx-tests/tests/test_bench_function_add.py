"""Persistent Python scaling guardrails for ``Function.__iadd__``.

PR #498 introduced the Python in-place path and PR #990 removed per-operation
Function normalization round-trips. ``small_many`` detects a return to
quadratic accumulator rebuilding; ``large_little`` holds the operation count
fixed and varies operand size to expose merge and wrapper-normalization cost.
The latter is Rust-internal characterization and remains in the manual Python
diagnostic suite; the Python ``small_many`` operator path is the guardrail.
"""

import pytest
from ommx import Function, Rng


@pytest.fixture(params=[100, 1000, 10_000])
def function_small_many(request):
    """Create many small functions with 5 terms each"""
    num_functions = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(num_functions):
        # Create function with 5 terms using the new random method
        func = Function.random(rng, num_terms=5, max_degree=3, max_id=num_functions)
        functions.append(func)
    return functions


@pytest.fixture(params=[100, 1000, 10_000])
def function_large_little(request):
    """Create few large functions with many terms"""
    num_terms = request.param
    rng = Rng()  # Create deterministic RNG
    functions = []
    for _ in range(3):  # Only 3 functions
        # Create function with many terms using the new random method
        func = Function.random(
            rng, num_terms=num_terms, max_degree=3, max_id=3 * num_terms
        )
        functions.append(func)
    return functions


def sum_function_functions(functions: list[Function]):
    """Sum many functions"""
    result = Function(0)
    for func in functions:
        result += func
    return result


@pytest.mark.benchmark_guardrail
@pytest.mark.benchmark
def test_sum_function_small_many(benchmark, function_small_many):
    """Measure repeated accumulation of many small Function wrappers."""
    benchmark(sum_function_functions, function_small_many)


@pytest.mark.benchmark_diagnostic
@pytest.mark.benchmark
def test_sum_function_large_little(benchmark, function_large_little):
    """Measure accumulation of a few high-term-count Function wrappers."""
    benchmark(sum_function_functions, function_large_little)
