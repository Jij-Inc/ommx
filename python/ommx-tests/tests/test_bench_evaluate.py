"""Python API benchmarks for ``Instance`` evaluation.

The persistent fixed-input guardrails use a small synthetic instance to detect
Python/PyO3 boundary regressions without repeatedly profiling heavy Rust
evaluation work. The ``supportcase10`` workloads reproduce issue #336 and stay
in the manual diagnostic suite for end-to-end profiling. Instance-level
algorithmic scaling is measured in ``rust/ommx/benches/evaluate.rs``.
"""

import pytest
from ommx import DecisionVariable, Instance, Rng
from ommx.dataset import miplib2017


@pytest.fixture
def boundary_instance():
    """Create a deterministic, small instance for Python boundary guardrails."""
    size = 32
    x = [DecisionVariable.binary(i) for i in range(size)]
    return Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={i: x[i] <= 1 for i in range(size)},
        sense=Instance.MINIMIZE,
    )


@pytest.fixture
def boundary_state(boundary_instance):
    rng = Rng()
    return boundary_instance.random_state(rng)


@pytest.fixture
def boundary_samples(boundary_instance):
    rng = Rng()
    return boundary_instance.random_samples(rng, num_different_samples=1, num_samples=1)


@pytest.fixture
def miplib_supportcase10():
    """Load the issue #336 reproduction instance for manual profiling."""
    return miplib2017("supportcase10")


@pytest.fixture
def miplib_state(miplib_supportcase10):
    rng = Rng()
    return miplib_supportcase10.random_state(rng)


@pytest.fixture
def miplib_samples(miplib_supportcase10):
    rng = Rng()
    return miplib_supportcase10.random_samples(
        rng, num_different_samples=1, num_samples=1
    )


def evaluate_state(instance, state):
    return instance.evaluate(state)


def evaluate_samples_batch(instance, samples):
    return instance.evaluate_samples(samples)


@pytest.mark.benchmark_guardrail
@pytest.mark.benchmark
def test_evaluate_python_boundary(benchmark, boundary_instance, boundary_state):
    """Detect fixed-cost regressions in one public ``Instance.evaluate`` call."""
    benchmark(evaluate_state, boundary_instance, boundary_state)


@pytest.mark.benchmark_guardrail
@pytest.mark.benchmark
def test_evaluate_samples_python_boundary(
    benchmark, boundary_instance, boundary_samples
):
    """Detect fixed-cost regressions in the public single-sample boundary."""
    benchmark(evaluate_samples_batch, boundary_instance, boundary_samples)


@pytest.mark.benchmark_diagnostic
@pytest.mark.benchmark
def test_evaluate_miplib(benchmark, miplib_supportcase10, miplib_state):
    """Profile end-to-end evaluation on the issue #336 MIPLIB input."""
    benchmark(evaluate_state, miplib_supportcase10, miplib_state)


@pytest.mark.benchmark_diagnostic
@pytest.mark.benchmark
def test_evaluate_samples_miplib(benchmark, miplib_supportcase10, miplib_samples):
    """Profile single-sample evaluation on the issue #336 MIPLIB input."""
    benchmark(evaluate_samples_batch, miplib_supportcase10, miplib_samples)
