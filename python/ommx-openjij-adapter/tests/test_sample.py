from ommx.v1 import Instance, DecisionVariable
from ommx_openjij_adapter import OMMXOpenJijSAAdapter


def test_minimize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    sample_set = OMMXOpenJijSAAdapter.sample(instance, num_reads=1)

    # x0 = x1 = 0 is minimum
    assert sample_set.extract_decision_variables("x", 0) == {(0,): 0.0, (1,): 0.0}


def test_maximize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    instance.as_minimization_problem()
    sample_set = OMMXOpenJijSAAdapter.sample(instance, num_reads=1)

    # x0 = x1 = 1 is maximum
    assert sample_set.extract_decision_variables("x", 0) == {(0,): 1.0, (1,): 1.0}
