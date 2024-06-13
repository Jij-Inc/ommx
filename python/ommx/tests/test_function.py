from ommx.v1 import Linear


def test_linear():
    # add to constants
    assert Linear(terms={}, constant=1) + 2 == Linear(terms={}, constant=3.0)
    assert 2 + Linear(terms={}, constant=1) == Linear(terms={}, constant=3.0)

    # mul to constants
    assert 2 * Linear(terms={1: 2, 2: 3}) == Linear(terms={1: 4, 2: 6})
    assert Linear(terms={1: 2, 2: 3}) * 2 == Linear(terms={1: 4, 2: 6})

    # add to linear
    assert Linear(terms={1: 2}) + Linear(terms={2: 3}) == Linear(terms={1: 2, 2: 3})
    assert Linear(terms={1: 2}) + Linear(terms={1: 3}) == Linear(terms={1: 5})
