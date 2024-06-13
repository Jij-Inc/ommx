from ommx.v1 import Linear


def test_linear_add():
    assert Linear(terms={}, constant=1) + 2 == Linear(terms={}, constant=3.0)
    assert 2 + Linear(terms={}, constant=1) == Linear(terms={}, constant=3.0)

    assert Linear(terms={1: 2}) + Linear(terms={2: 3}) == Linear(terms={1: 2, 2: 3})
    assert Linear(terms={1: 2}) + Linear(terms={1: 3}) == Linear(terms={1: 5})
