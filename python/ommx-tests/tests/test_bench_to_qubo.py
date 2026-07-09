"""Persistent Python scaling guardrails for ``Instance.to_qubo``.

``to_qubo`` is a Python driver API that sequences several Rust transformations,
so the public boundary is part of the measured operation. The pseudo-Boolean
family originates from issue #404 and PR #495 and varies the number of terms to
guard the expected output-sensitive cost: penalizing an N-term equality creates
Theta(N^2) QUBO coefficients, so conversion should not grow beyond quadratic.
The bounded 10/32/100 inputs retain that trend without restoring the 1,000-term
profile that dominated the CodSpeed workflow. The defensive copy is included
because the driver consumes mutable instance state.
"""

import pytest
import random
from copy import deepcopy
from ommx import DecisionVariable, Instance


pytestmark = pytest.mark.benchmark_guardrail


@pytest.fixture
def small():
    x = [
        DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i])
        for i in range(2)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={0: x[0] + 2 * x[1] <= 3},
        sense=Instance.MAXIMIZE,
    )
    return instance


@pytest.fixture(params=[10, 32, 100])
def pseudo_boolean_inequality(request):
    num_terms: int = request.param
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(num_terms)]
    random.seed(123456789)
    expr = sum(random.choice([num_terms // 2, 1, -1]) * x[i] for i in range(num_terms))
    threshold = num_terms // 2
    instance = Instance.from_components(
        decision_variables=x,
        objective=0,
        constraints={0: expr <= threshold},  # type: ignore
        sense=Instance.MAXIMIZE,
    )
    return instance


def to_qubo(instance: Instance):
    """
    Run `Instance.to_qubo` without modifying the original instance.

    Although `fixture`s are created per test, pytest-benchmark (and codspeed) will

    1. Initialize the input of test function
    2. Call the test function multiple times with **this** input to measure the performance

    This means that if the first run modifies the input, the subsequent runs will be affected.
    """
    new_instance = deepcopy(
        instance
    )  # Create a new instance since `to_qubo` modifies the instance
    new_instance.to_qubo()


@pytest.mark.benchmark
def test_to_qubo_small(small: Instance):
    """Track fixed boundary cost for a tiny integer-constrained model."""
    to_qubo(small)


@pytest.mark.benchmark
def test_to_qubo_pbi(pseudo_boolean_inequality: Instance):
    """Track scaling with the pseudo-Boolean inequality term count."""
    to_qubo(pseudo_boolean_inequality)
