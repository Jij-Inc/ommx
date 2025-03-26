from ommx.v1 import Instance, DecisionVariable
from ommx_openjij_adapter import OMMXOpenJijSAAdapter
import pytest


def binary_no_constraint_minimize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    ans = {(0,): 0.0, (1,): 0.0}
    return pytest.param(instance, ans, id="binary_no_constraint_minimize")


def binary_no_constraint_maximize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    ans = {(0,): 1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="binary_no_constraint_maximize")


def binary_equality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2,
        constraints=[x1 * x2 == 0],
        sense=Instance.MAXIMIZE,
    )

    # x0 = x2 = 1, x1 = 0 is maximum
    ans = {(0,): 1.0, (1,): 0.0, (2,): 1.0}
    return pytest.param(instance, ans, id="binary_equality")


def binary_inequality():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])
    x2 = DecisionVariable.binary(2, name="x", subscripts=[2])

    instance = Instance.from_components(
        decision_variables=[x0, x1, x2],
        objective=x0 + 2 * x1 + 3 * x2,
        constraints=[x0 + x1 + x2 <= 2],
        sense=Instance.MAXIMIZE,
    )

    # x1 = x2 = 1, x0 = 0 is maximum
    ans = {(0,): 0.0, (1,): 1.0, (2,): 1.0}
    return pytest.param(instance, ans, id="binary_inequality")


def integer_equality():
    x0 = DecisionVariable.integer(0, name="x", lower=-1, upper=1, subscripts=[0])
    x1 = DecisionVariable.integer(1, name="x", lower=-1, upper=1, subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + 2 * x1,
        constraints=[x0 + x1 == 0],
        sense=Instance.MAXIMIZE,
    )

    # x1 = -x0 = 1 is maximum
    ans = {(0,): -1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="integer_equality")


def integer_inequality():
    x0 = DecisionVariable.integer(0, name="x", lower=-1, upper=1, subscripts=[0])
    x1 = DecisionVariable.integer(1, name="x", lower=-1, upper=1, subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + 2 * x1,
        constraints=[x0 + x1 <= 0],
        sense=Instance.MAXIMIZE,
    )

    # x1 = -x0 = 1 is maximum
    ans = {(0,): -1.0, (1,): 1.0}
    return pytest.param(instance, ans, id="integer_inequality")


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        binary_no_constraint_maximize(),
        binary_equality(),
        binary_inequality(),
        integer_equality(),
        integer_inequality(),
    ],
)
def test_sample(instance, ans):
    sample_set = OMMXOpenJijSAAdapter.sample(
        instance, num_reads=1, uniform_penalty_weight=3.0, seed=12345
    )
    assert sample_set.extract_decision_variables("x", 0) == ans


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        binary_no_constraint_maximize(),
        binary_equality(),
        binary_inequality(),
        integer_equality(),
        integer_inequality(),
    ],
)
def test_solve(instance, ans):
    solution = OMMXOpenJijSAAdapter.solve(
        instance, num_reads=1, uniform_penalty_weight=3.0, seed=12345
    )
    assert solution.extract_decision_variables("x") == ans


@pytest.mark.parametrize(
    "instance, ans",
    [
        binary_no_constraint_minimize(),
        binary_no_constraint_maximize(),
        binary_equality(),
        binary_inequality(),
        integer_equality(),
        integer_inequality(),
    ],
)
def test_sample_twice(instance, ans):
    test_sample(instance, ans)
    test_sample(instance, ans)
