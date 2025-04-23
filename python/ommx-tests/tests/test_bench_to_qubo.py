import pytest
import random
from copy import deepcopy
from ommx.v1 import DecisionVariable, Instance


@pytest.fixture
def small():
    x = [
        DecisionVariable.integer(i, lower=0, upper=2, name="x", subscripts=[i])
        for i in range(2)
    ]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[(x[0] + 2 * x[1] <= 3).set_id(0)],
        sense=Instance.MAXIMIZE,
    )
    return instance


@pytest.fixture(params=[10, 100])
def pseudo_boolean_inequality(request):
    num_terms: int = request.param
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(num_terms)]
    random.seed(123456789)
    expr = sum(random.choice([num_terms // 2, 1, -1]) * x[i] for i in range(num_terms))
    threshold = num_terms // 2
    instance = Instance.from_components(
        decision_variables=x,
        objective=0,
        constraints=[(expr <= threshold).set_id(0)],  # type: ignore
        sense=Instance.MAXIMIZE,
    )
    return instance


def to_qubo(instance):
    """
    Run `Instance.to_qubo` without modifying the original instance.

    Although `fixture`s are created par test call, pytest-benchmark (and codspeed) will

    1. Initialize the input of test function
    2. Call the test function multiple times with **this** input to measure the performance

    This means that if the first run modifies the input, the subsequent runs will be affected.
    Thus we need to create a deep copy of the input before calling the test function.
    """
    new_instance = deepcopy(instance)
    return new_instance.to_qubo()


@pytest.mark.benchmark
def test_to_qubo_small(small):
    to_qubo(small)


@pytest.mark.benchmark
def test_to_qubo_pbi(pseudo_boolean_inequality):
    to_qubo(pseudo_boolean_inequality)
