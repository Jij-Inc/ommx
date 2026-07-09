"""Python API benchmarks for Instance evaluation.

These benchmarks measure end-to-end evaluation through the public Python
bindings on a real MIPLIB instance. Large-scale evaluation throughput belongs
in Rust SDK benchmarks; this file keeps representative Python-call coverage.
"""

import pytest
from ommx import Rng
from ommx.dataset import miplib2017


@pytest.fixture
def miplib_supportcase10():
    """Load MIPLIB supportcase10 instance for benchmarking"""
    return miplib2017("supportcase10")


@pytest.fixture
def random_state(miplib_supportcase10):
    """Generate a random state for evaluation"""
    rng = Rng()
    return miplib_supportcase10.random_state(rng)


@pytest.fixture(params=[(1, 1)], ids=["single-sample"])
def samples(request, miplib_supportcase10):
    """Generate a representative sample set for Python E2E benchmarking."""
    num_different_samples, num_samples = request.param
    rng = Rng()
    return miplib_supportcase10.random_samples(
        rng, num_different_samples=num_different_samples, num_samples=num_samples
    )


def evaluate_state(instance, state):
    """Evaluate a single state"""
    return instance.evaluate(state)


def evaluate_samples_batch(instance, samples):
    """Evaluate samples using evaluate_samples method"""
    return instance.evaluate_samples(samples)


@pytest.mark.benchmark
def test_evaluate(benchmark, miplib_supportcase10, random_state):
    """Measure one public `Instance.evaluate` call on a real instance."""
    benchmark(evaluate_state, miplib_supportcase10, random_state)


@pytest.mark.benchmark
def test_evaluate_samples(benchmark, miplib_supportcase10, samples):
    """Measure one representative public `Instance.evaluate_samples` call."""
    benchmark(evaluate_samples_batch, miplib_supportcase10, samples)
