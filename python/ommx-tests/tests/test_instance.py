from ommx.v1 import Instance, DecisionVariable, Function


def test_set_objective():
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    assert instance.objective.almost_equal(Function(sum(x)))

    instance.objective = x[1]
    assert instance.objective.almost_equal(Function(x[1]))
