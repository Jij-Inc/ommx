from ommx.v1 import Instance, DecisionVariable


def test_evaluate_samples_type_check():
    """
    Reported case in bug report https://github.com/Jij-Inc/ommx/issues/393
    """
    x = [DecisionVariable.binary(i) for i in range(3)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0] + 2 * x[1] + 3 * x[2],
        constraints=[x[1] + x[2] <= 1],
        sense=Instance.MAXIMIZE,
    )

    samples = [
        {0: 1, 1: 0, 2: 0},
        {0: 0, 1: 0, 2: 1},
        {0: 1, 1: 1, 2: 0},
    ]
    sample_set = instance.evaluate_samples(samples)
