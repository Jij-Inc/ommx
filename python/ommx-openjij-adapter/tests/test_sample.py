from ommx.v1 import Instance, DecisionVariable
import ommx_openjij_adapter as adapter


def test_minimize():
    x0 = DecisionVariable.binary(0, name="x", subscripts=[0])
    x1 = DecisionVariable.binary(1, name="x", subscripts=[1])

    instance = Instance.from_components(
        decision_variables=[x0, x1],
        objective=x0 + x1,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    samples = adapter.sample_qubo_sa(instance, num_reads=1)
    sample_set = instance.evaluate_samples(samples)

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
    samples = adapter.sample_qubo_sa(instance, num_reads=1)
    sample_set = instance.evaluate_samples(samples)

    # x0 = x1 = 1 is maximum
    assert sample_set.extract_decision_variables("x", 0) == {(0,): 1.0, (1,): 1.0}
