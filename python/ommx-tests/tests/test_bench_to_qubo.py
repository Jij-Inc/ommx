import pytest
from statistics import median
from ommx.v1 import DecisionVariable, Instance
from copy import deepcopy
import random


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
    new_instance = deepcopy(instance)
    return new_instance.to_qubo()


def test_to_qubo(benchmark, small):
    result = benchmark(to_qubo, small)
    assert result is not None


def test_to_qubo_pseudo_boolean_inequality(benchmark, pseudo_boolean_inequality):
    result = benchmark(
        to_qubo,
        pseudo_boolean_inequality,
    )
    assert result is not None


@pytest.mark.benchmark
def test_no_fixture():
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
    instance.to_qubo()
