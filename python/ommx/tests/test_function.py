from ommx.v1 import Linear, DecisionVariable, Quadratic


def assert_eq(lhs, rhs):
    assert lhs.almost_equal(rhs), f"{lhs} != {rhs}"


def test_decision_variable():
    assert_eq(DecisionVariable.binary(1) + 2, Linear(terms={1: 1}, constant=2))
    assert_eq(3 + DecisionVariable.binary(1), Linear(terms={1: 1}, constant=3))
    assert_eq(DecisionVariable.binary(1) * 2, Linear(terms={1: 2}))
    assert_eq(3 * DecisionVariable.binary(1), Linear(terms={1: 3}))


def test_linear():
    # add to constants
    assert_eq(Linear(terms={}, constant=1) + 2, Linear(terms={}, constant=3.0))
    assert_eq(2 + Linear(terms={}, constant=1), Linear(terms={}, constant=3.0))

    # mul to constants
    assert_eq(2 * Linear(terms={1: 2, 2: 3}), Linear(terms={1: 4, 2: 6}))
    assert_eq(Linear(terms={1: 2, 2: 3}) * 2, Linear(terms={1: 4, 2: 6}))

    # add to decision variable
    assert_eq(
        Linear(terms={1: 2}, constant=3) + DecisionVariable.binary(2),
        Linear(terms={1: 2, 2: 1}, constant=3),
    )
    assert_eq(
        DecisionVariable.binary(2) + Linear(terms={1: 2}, constant=3),
        Linear(terms={1: 2, 2: 1}, constant=3),
    )

    # add to linear
    assert_eq(Linear(terms={1: 2}) + Linear(terms={2: 3}), Linear(terms={1: 2, 2: 3}))
    assert_eq(Linear(terms={1: 2}) + Linear(terms={1: 3}), Linear(terms={1: 5}))


def test_quadratic():
    # add to constants
    assert_eq(
        Quadratic(columns=[1], rows=[0], values=[1.0]) + 3,
        Quadratic(
            columns=[1], rows=[0], values=[1.0], linear=Linear(terms={}, constant=3)
        ),
    )
    assert_eq(
        3 + Quadratic(columns=[1], rows=[0], values=[1.0]),
        Quadratic(
            columns=[1], rows=[0], values=[1.0], linear=Linear(terms={}, constant=3)
        ),
    )

    # x0 * x1 = x1 * x0
    assert_eq(
        Quadratic(columns=[1], rows=[0], values=[1.0]),
        Quadratic(columns=[0], rows=[1], values=[1.0]),
    )
    # x1 * x0 + 2 * x2 * x3 = x0 * x1 + 2 * x3 * x2
    assert_eq(
        Quadratic(columns=[1, 2], rows=[0, 3], values=[1.0, 2.0]),
        Quadratic(columns=[0, 3], rows=[1, 2], values=[1.0, 2.0]),
    )
