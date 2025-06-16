# FIXME: Use test case generator like Hypothesis

from ommx.v1 import Linear, DecisionVariable, Quadratic, Polynomial, Function


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

    # test in-place add
    l = Linear(terms={1: 2}, constant=3)
    original_id = id(l)
    l += Linear(terms={2: 3})
    assert id(l) == original_id  # Verify it's the same object
    assert_eq(l, Linear(terms={1: 2, 2: 3}, constant=3))


def test_quadratic():
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)

    # DecisionVariable * DecisionVariable
    assert_eq(x1 * x1, Quadratic(columns=[1], rows=[1], values=[1.0]))
    assert_eq(x1 * x2, Quadratic(columns=[1], rows=[2], values=[1.0]))
    # DecisionVariable * Linear
    assert_eq(2.0 * x1 * x2, Quadratic(columns=[1], rows=[2], values=[2.0]))
    assert_eq(x1 * 2.0 * x2, Quadratic(columns=[1], rows=[2], values=[2.0]))
    assert_eq(x1 * x2 * 2.0, Quadratic(columns=[1], rows=[2], values=[2.0]))
    assert_eq(
        x1 * (x2 + 1),
        Quadratic(
            columns=[1], rows=[2], values=[1.0], linear=Linear(terms={1: 1}, constant=0)
        ),
    )
    assert_eq(
        (x2 + 1) * x1,
        Quadratic(
            columns=[1], rows=[2], values=[1.0], linear=Linear(terms={1: 1}, constant=0)
        ),
    )

    assert_eq(
        x1 * x2 + 2,
        Quadratic(
            columns=[1],
            rows=[2],
            values=[1.0],
            linear=Linear(terms={}, constant=2),
        ),
    )
    assert_eq(
        2 + x1 * x2,
        Quadratic(
            columns=[1],
            rows=[2],
            values=[1.0],
            linear=Linear(terms={}, constant=2),
        ),
    )
    assert_eq(
        x1 * x2 + x3 + 2,
        Quadratic(
            columns=[1],
            rows=[2],
            values=[1.0],
            linear=Linear(terms={3: 1}, constant=2),
        ),
    )
    assert_eq(
        x1 * x2 + (x3 + 2),
        Quadratic(
            columns=[1],
            rows=[2],
            values=[1.0],
            linear=Linear(terms={3: 1}, constant=2),
        ),
    )
    assert_eq(
        (x3 + 2) + x1 * x2,
        Quadratic(
            columns=[1],
            rows=[2],
            values=[1.0],
            linear=Linear(terms={3: 1}, constant=2),
        ),
    )

    assert_eq(x1 * x2 + x1 * x2, 2 * x1 * x2)

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


def test_polynomial():
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)

    # DecisionVariable * DecisionVariable
    assert_eq(x1 * x1 * x1, Polynomial(terms={(1, 1, 1): 1.0}))
    assert_eq(x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 1.0}))
    assert_eq(x1 * x3 * x2, Polynomial(terms={(1, 2, 3): 1.0}))
    assert_eq(2.0 * x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 2.0}))
    assert_eq(x1 * 2.0 * x2 * x3, Polynomial(terms={(1, 2, 3): 2.0}))
    assert_eq(x1 * x2 * 2.0 * x3, Polynomial(terms={(1, 2, 3): 2.0}))

    assert_eq(x1 * x2 * x3 + 2, Polynomial(terms={(1, 2, 3): 1.0, (): 2.0}))
    assert_eq(2 + x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 1.0, (): 2.0}))

    assert_eq(x1 * x2 * x3 + x1, Polynomial(terms={(1, 2, 3): 1.0, (1,): 1.0}))
    assert_eq(x1 + x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 1.0, (1,): 1.0}))
    assert_eq(x1 * x2 * x3 + 2.0 * x1, Polynomial(terms={(1, 2, 3): 1.0, (1,): 2.0}))
    assert_eq(2.0 * x1 + x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 1.0, (1,): 2.0}))
    assert_eq(
        x1 * x2 * x3 + 2.0 * x1 * x2, Polynomial(terms={(1, 2, 3): 1.0, (1, 2): 2.0})
    )
    assert_eq(
        2.0 * x1 * x2 + x1 * x2 * x3, Polynomial(terms={(1, 2, 3): 1.0, (1, 2): 2.0})
    )

    assert_eq(
        x1 * x2 * x3 + x1 * x2 * x3,
        2 * x1 * x2 * x3,
    )


def test_function():
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)

    assert_eq(Function(x1) + Function(3.0), Function(x1 + 3.0))
    assert_eq(Function(x1) + Function(x2), Function(x1 + x2))
    assert_eq(Function(x1) * Function(x2), Function(x1 * x2))
    assert_eq(Function(x1 * x2) * Function(x3), Function(x1 * x2 * x3))
