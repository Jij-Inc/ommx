from ommx.v1 import Instance


def test_solution_sense_minimize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    solution = instance.evaluate({})
    assert solution.sense == Instance.MINIMIZE


def test_solution_sense_maximize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[],
        sense=Instance.MAXIMIZE,
    )
    solution = instance.evaluate({})
    assert solution.sense == Instance.MAXIMIZE
