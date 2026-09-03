from ommx import Instance, Sense


def test_solution_sense_minimize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Sense.Minimize,
    )
    solution = instance.evaluate({})
    assert solution.sense == Sense.Minimize


def test_solution_sense_maximize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Sense.Maximize,
    )
    solution = instance.evaluate({})
    assert solution.sense == Sense.Maximize
