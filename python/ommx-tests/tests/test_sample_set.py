from ommx import Instance, DecisionVariable, Sense


def test_evaluate_samples_type_check():
    """
    Reported case in bug report https://github.com/Jij-Inc/ommx/issues/393
    """
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + 2 * x[1] + 3 * x[2],
        constraints={0: x[1] + x[2] <= 1},
        sense=Sense.Maximize,
    )

    samples = [
        {0: 1, 1: 0, 2: 0},
        {0: 0, 1: 0, 2: 1},
        {0: 1, 1: 1, 2: 0},
    ]
    sample_set = instance.evaluate_samples(samples)

    assert sample_set.extract_decision_variables("x", 0) == {(0,): 1, (1,): 0, (2,): 0}
    assert sample_set.extract_decision_variables("x", 1) == {(0,): 0, (1,): 0, (2,): 1}
    assert sample_set.extract_decision_variables("x", 2) == {(0,): 1, (1,): 1, (2,): 0}


def test_sample_set_sense_minimize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Sense.Minimize,
    )
    sample_set = instance.evaluate_samples([{}])
    assert sample_set.sense == Sense.Minimize

    # Check that the sense is correct when creating a Solution
    # from a SampleSet using `.best_feasible`.
    solution = sample_set.best_feasible
    assert solution.sense == Sense.Minimize


def test_sample_set_sense_maximize():
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints={},
        sense=Sense.Maximize,
    )
    sample_set = instance.evaluate_samples([{}])
    assert sample_set.sense == Sense.Maximize

    # Check that the sense is correct when creating a Solution
    # from a SampleSet using `.get()`.
    sokution = sample_set.get(0)
    assert sokution.sense == Sense.Maximize
