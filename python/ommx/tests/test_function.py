from ommx.v1 import Linear, DecisionVariable


def test_decision_variable():
    assert DecisionVariable.binary(1) + 2 == Linear(terms={1: 1}, constant=2)
    assert 3 + DecisionVariable.binary(1) == Linear(terms={1: 1}, constant=3)
    assert DecisionVariable.binary(1) * 2 == Linear(terms={1: 2})
    assert 3 * DecisionVariable.binary(1) == Linear(terms={1: 3})


def test_linear():
    # add to constants
    assert Linear(terms={}, constant=1) + 2 == Linear(terms={}, constant=3.0)
    assert 2 + Linear(terms={}, constant=1) == Linear(terms={}, constant=3.0)

    # mul to constants
    assert 2 * Linear(terms={1: 2, 2: 3}) == Linear(terms={1: 4, 2: 6})
    assert Linear(terms={1: 2, 2: 3}) * 2 == Linear(terms={1: 4, 2: 6})

    # add to decision variable
    assert Linear(terms={1: 2}, constant=3) + DecisionVariable.binary(2) == Linear(
        terms={1: 2, 2: 1}, constant=3
    )
    assert DecisionVariable.binary(2) + Linear(terms={1: 2}, constant=3) == Linear(
        terms={1: 2, 2: 1}, constant=3
    )

    # add to linear
    assert Linear(terms={1: 2}) + Linear(terms={2: 3}) == Linear(terms={1: 2, 2: 3})
    assert Linear(terms={1: 2}) + Linear(terms={1: 3}) == Linear(terms={1: 5})
