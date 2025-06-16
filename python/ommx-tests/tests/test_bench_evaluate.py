import pytest
from ommx.v1 import Rng
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


def evaluate_state(instance, state):
    """Evaluate a single state"""
    return instance.evaluate(state)


@pytest.mark.benchmark
def test_evaluate(benchmark, miplib_supportcase10, random_state):
    """Benchmark individual evaluate call"""
    benchmark(evaluate_state, miplib_supportcase10, random_state)
