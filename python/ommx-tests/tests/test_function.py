# FIXME: Use test case generator like Hypothesis

import math
import sys

import numpy as np
import pytest

from ommx import (
    Bound,
    DecisionVariable,
    Function,
    Instance,
    Linear,
    Parameter,
    Polynomial,
    Quadratic,
    Rng,
)


def assert_eq(lhs, rhs):
    assert lhs.almost_equal(rhs), f"{lhs} != {rhs}"


def test_function_random_uses_polynomial_parameter_space():
    function = Function.random(Rng(), num_terms=5, max_degree=3, max_id=10)
    assert function.degree() is not None
    assert function.num_terms() == 5


def test_decision_variable():
    assert_eq(DecisionVariable.binary(1) + 2, Linear(terms={1: 1}, constant=2))
    assert_eq(3 + DecisionVariable.binary(1), Linear(terms={1: 1}, constant=3))
    assert_eq(DecisionVariable.binary(1) * 2, Linear(terms={1: 2}))
    assert_eq(3 * DecisionVariable.binary(1), Linear(terms={1: 3}))


def test_python_arithmetic_raises_on_coefficient_overflow():
    huge = sys.float_info.max
    x = DecisionVariable.binary(1)

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = huge * x + huge * x

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = huge * x * huge


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
    linear_instance = Linear(terms={1: 2}, constant=3)
    original_id = id(linear_instance)
    linear_instance += Linear(terms={2: 3})
    assert id(linear_instance) == original_id  # Verify it's the same object
    assert_eq(linear_instance, Linear(terms={1: 2, 2: 3}, constant=3))


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

    # test in-place add
    quad_instance = Quadratic(columns=[1], rows=[2], values=[2.0])
    original_id = id(quad_instance)
    quad_instance += Quadratic(columns=[3], rows=[4], values=[3.0])
    assert id(quad_instance) == original_id  # Verify it's the same object
    assert_eq(quad_instance, Quadratic(columns=[1, 3], rows=[2, 4], values=[2.0, 3.0]))


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

    # test in-place add
    poly_instance = Polynomial(terms={(1, 2): 2.0})
    original_id = id(poly_instance)
    poly_instance += Polynomial(terms={(3, 4): 3.0})
    assert id(poly_instance) == original_id  # Verify it's the same object
    assert_eq(poly_instance, Polynomial(terms={(1, 2): 2.0, (3, 4): 3.0}))


def test_function():
    x1 = DecisionVariable.binary(1)
    x2 = DecisionVariable.binary(2)
    x3 = DecisionVariable.binary(3)

    assert_eq(Function(x1) + Function(3.0), Function(x1 + 3.0))
    assert_eq(Function(x1) + Function(x2), Function(x1 + x2))
    assert_eq(Function(x1) * Function(x2), Function(x1 * x2))
    assert_eq(Function(x1 * x2) * Function(x3), Function(x1 * x2 * x3))

    # test in-place add
    func_instance = Function(x1)
    original_id = id(func_instance)
    func_instance += Function(x2)
    assert id(func_instance) == original_id  # Verify it's the same object
    assert_eq(func_instance, Function(x1 + x2))


def test_function_non_polynomial_operators():
    x = Function(DecisionVariable.continuous(1))
    y = Function(DecisionVariable.continuous(2))

    absolute = abs(x - 3)
    assert absolute.type_name == "Expression"
    assert absolute.evaluate({1: 1}) == 2

    sign = (x - 3).signum()
    assert sign.evaluate({1: 1}) == -1
    assert sign.evaluate({1: 3}) == 0
    assert sign.evaluate({1: 5}) == 1

    minimum = x.minimum(y)
    maximum = x.maximum(y)
    assert minimum.type_name == "Expression"
    assert maximum.type_name == "Expression"
    assert minimum.evaluate({1: 2, 2: 5}) == 2
    assert maximum.evaluate({1: 2, 2: 5}) == 5

    quotient = x / (y + 1)
    reverse_quotient = 12 / (x + 1)
    assert quotient.type_name == "Expression"
    assert quotient.evaluate({1: 6, 2: 2}) == 2
    assert reverse_quotient.evaluate({1: 2}) == 4

    power = x**2
    method_power = x.powi(3)
    assert power.type_name == "Expression"
    assert method_power.type_name == "Expression"
    assert power.evaluate({1: 3}) == 9
    assert method_power.evaluate({1: 2}) == 8
    assert pow(x, 2, None).evaluate({1: 3}) == 9

    original_id = id(absolute)
    absolute += y
    assert id(absolute) == original_id
    assert absolute.evaluate({1: 1, 2: 5}) == 7


def test_function_reverse_associative_operators_preserve_left_operand_order():
    x = Function(DecisionVariable.continuous(1))
    overflow = Function(Linear(terms={1: sys.float_info.max}))
    reciprocal_at_two = 1 / (x - 2)

    # The left polynomial overflows at x=2, while the right operand is also
    # undefined there. Ordered associative evaluation must report the left
    # failure first even when Python dispatches through the reverse operator.
    for expression in (
        reciprocal_at_two.__radd__(overflow),
        reciprocal_at_two.__rmul__(overflow),
    ):
        with pytest.raises(
            ValueError, match="polynomial function produced a non-finite value"
        ):
            expression.evaluate({1: 2})


def test_compact_types_reverse_magic_methods_preserve_function_lhs_order():
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=[DecisionVariable.continuous(2)],
        constraints={},
    )
    attached = instance.decision_variables[0]
    operands = [
        (DecisionVariable.continuous(2), Linear),
        (attached, Linear),
        (Parameter(2), Linear),
        (Linear(terms={2: 1}), Linear),
        (Quadratic(columns=[2], rows=[2], values=[1]), Quadratic),
        (Polynomial(terms={(2,): 1}), Polynomial),
    ]
    undefined_lhs = 1 / Function(Linear(terms={999: 1}))

    for operand, compact_type in operands:
        for method_name in ("__radd__", "__rsub__", "__rmul__"):
            expression = getattr(operand, method_name)(undefined_lhs)
            assert isinstance(expression, Function)
            # The Function lhs is undefined, while the compact rhs evaluates to
            # a non-finite value. The lhs error must be reported first.
            with pytest.raises(ValueError, match="division by zero"):
                expression.evaluate({999: 0, 2: math.inf})

            # Non-Function inputs retain each compact type's existing result type.
            assert isinstance(getattr(operand, method_name)(1), compact_type)


def test_function_power_rejects_modulo():
    x = Function(DecisionVariable.continuous(1))

    with pytest.raises(TypeError, match="modular exponentiation is not supported"):
        pow(x, 2, 3)  # pyright: ignore[reportCallIssue, reportArgumentType]


def test_function_power_requires_signed_32_bit_integer_exponent():
    x = Function(DecisionVariable.continuous(1))

    with pytest.raises(TypeError):
        _ = x ** Function(2)  # pyright: ignore[reportOperatorIssue]
    with pytest.raises(TypeError):
        _ = x**2.0  # pyright: ignore[reportOperatorIssue]
    with pytest.raises(TypeError):
        _ = 2**x  # pyright: ignore[reportOperatorIssue]
    with pytest.raises(OverflowError):
        x.powi(2**31)


def test_function_evaluation_reports_undefined_integer_power_domain():
    x = Function(DecisionVariable.continuous(1))
    reciprocal = 1 / x

    with pytest.raises(ValueError, match="division by zero"):
        reciprocal.evaluate({1: 0})

    with pytest.raises(ValueError, match="negative integer exponent"):
        (x**-1).evaluate({1: 0})

    assert (x**0).evaluate({1: 0}) == 1
    assert (x**2).evaluate({1: -2}) == 4
    assert (x**3).evaluate({1: -2}) == -8

    # Multiplication by zero must not erase the domain of a partial function.
    with pytest.raises(ValueError, match="division by zero"):
        (0 * reciprocal).evaluate({1: 0})


def test_function_zero_sensitive_operations_defer_to_evaluation_atol():
    x = Function(DecisionVariable.continuous(1))
    tiny = 1e-8
    coarse_atol = 1e-6
    fine_atol = 1e-9

    signum = x.signum().partial_evaluate({1: tiny}, atol=coarse_atol)
    assert signum.type_name == "Expression"
    assert signum.evaluate({}, atol=coarse_atol) == 0
    assert signum.evaluate({}, atol=fine_atol) == 1

    reciprocal = (1 / x).partial_evaluate({1: tiny}, atol=coarse_atol)
    assert reciprocal.type_name == "Expression"
    with pytest.raises(ValueError, match="division by zero"):
        reciprocal.evaluate({}, atol=coarse_atol)
    assert reciprocal.evaluate({}, atol=fine_atol) == pytest.approx(1 / tiny)

    inverse = x.powi(-1).partial_evaluate({1: tiny}, atol=coarse_atol)
    assert inverse.type_name == "Expression"
    with pytest.raises(ValueError, match="negative integer exponent"):
        inverse.evaluate({}, atol=coarse_atol)
    assert inverse.evaluate({}, atol=fine_atol) == pytest.approx(1 / tiny)


def test_function_evaluation_rejects_non_finite_results():
    function = Function(Linear(terms={1: sys.float_info.max}))

    with pytest.raises(ValueError, match="produced a non-finite value"):
        function.evaluate({1: 2})


def test_non_polynomial_function_has_no_polynomial_metadata():
    function = abs(Function(DecisionVariable.continuous(1)))

    assert function.degree() is None
    assert function.num_terms() is None
    assert function.as_linear() is None
    assert function.as_quadratic() is None

    for attribute in ("terms", "linear_terms", "quadratic_terms", "constant_term"):
        with pytest.raises(
            TypeError, match=f"Function\\.{attribute} is only available for polynomial"
        ):
            getattr(function, attribute)

    with pytest.raises(
        TypeError, match="Function\\.content_factor is only available for polynomial"
    ):
        function.content_factor()


def test_function_arithmetic_raises_on_coefficient_overflow():
    huge = sys.float_info.max
    f = Function(Linear(terms={1: huge}))

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = f + f

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = f * f


def test_content_factor_raises_value_error_for_unrepresentable_coefficient():
    f = Function(Linear(terms={1: sys.float_info.max}))

    with pytest.raises(ValueError, match="Cannot approximate coefficient"):
        f.content_factor()


def test_comparison_raises_value_error_on_coefficient_overflow():
    huge = sys.float_info.max
    lhs = Function(Linear(terms={1: huge}))
    rhs = Function(Linear(terms={1: -huge}))

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = lhs == rhs
    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = lhs <= rhs
    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = lhs >= rhs

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        _ = Linear(terms={1: huge}) <= Linear(terms={1: -huge})


def test_function_terms_zero():
    zero = Function(0)
    assert zero.terms == {}


def test_function_terms_non_zero_constant():
    constant = Function(2)
    assert constant.terms == {(): 2.0}


def test_function_terms_linear():
    homogeneous_linear = Function(Linear(terms={0: 9, 2: 8.7}))
    assert homogeneous_linear.terms == {(0,): 9.0, (2,): 8.7}

    linear = Function(Linear(terms={0: 9}, constant=1))
    assert linear.terms == {(0,): 9.0, (): 1.0}


def test_function_terms_quadratic():
    homogeneous_quadratic = Function(
        Quadratic(
            columns=[0, 1, 2],
            rows=[3, 4, 5],
            values=[6, 7, 8.9],
        )
    )
    assert homogeneous_quadratic.terms == {
        (0, 3): 6.0,
        (1, 4): 7.0,
        (2, 5): 8.9,
    }

    quadratic = Function(
        Quadratic(
            columns=[0],
            rows=[1],
            values=[2.3],
            linear=Linear(terms={4: 5}, constant=6.7),
        )
    )
    assert quadratic.terms == {
        (0, 1): 2.3,
        (4,): 5.0,
        (): 6.7,
    }


def test_function_terms_polynomial():
    homogeneous_polynomial = Function(
        Polynomial(
            terms={
                (0, 1, 2): 3.4,
                (5, 6, 7): 8,
            }
        )
    )
    assert homogeneous_polynomial.terms == {
        (0, 1, 2): 3.4,
        (5, 6, 7): 8.0,
    }

    polynomial = Function(
        Polynomial(
            terms={
                (0, 1, 2): 3,
                (4, 5): 6.7,
                (8,): 9,
                (): 10,
            }
        )
    )
    assert polynomial.terms == {
        (0, 1, 2): 3.0,
        (4, 5): 6.7,
        (8,): 9.0,
        (): 10.0,
    }


def test_function_from_numpy_int64():
    """numpy.int64 should be accepted as a constant value."""
    i = np.int64(3)
    f = Function(i)
    assert_eq(f, Function(3))


def test_function_from_numpy_float64():
    """numpy.float64 should be accepted as a constant value."""
    x = np.float64(2.5)
    f = Function(x)
    assert_eq(f, Function(2.5))


def test_function_evaluate_bound():
    # Constant: bound is a degenerate interval at the constant value.
    assert Function(3.5).evaluate_bound({}) == Bound(3.5, 3.5)

    # Linear: 2*x1 + 3 over x1 in [0, 2] -> [3, 7].
    f = Function(Linear(terms={1: 2}, constant=3))
    bound = f.evaluate_bound({1: Bound(0.0, 2.0)})
    assert bound.lower <= 3.0
    assert bound.upper >= 7.0

    # Squared term via interval-power semantics, not naive interval square.
    # x1 * x1 with x1 in [-2, 3] -> [0, 9] (not [-6, 9]).
    q = Function(Quadratic(columns=[1], rows=[1], values=[1.0]))
    bound = q.evaluate_bound({1: Bound(-2.0, 3.0)})
    assert bound.lower <= 0.0
    assert bound.upper >= 9.0

    # Missing variable ID is treated as unbounded.
    unbounded = f.evaluate_bound({})
    assert math.isinf(unbounded.lower) and unbounded.lower < 0
    assert math.isinf(unbounded.upper) and unbounded.upper > 0


def test_function_evaluate_bound_is_not_tight():
    # evaluate_bound is a term-wise (monomial-wise) interval evaluation, which
    # is a sound over-approximation but not tight when terms share variables.
    #
    # f = x^2 - x with x in [0, 1] has true range [-1/4, 0] (minimum at x=1/2,
    # maximum at x=0 or x=1), but term-wise we get [0,1] + (-[0,1]) = [-1, 1].
    f = Function(Quadratic(columns=[1], rows=[1], values=[1.0])) + Function(
        Linear(terms={1: -1})
    )
    b = f.evaluate_bound({1: Bound(0.0, 1.0)})
    assert b.lower <= -1.0
    assert b.upper >= 1.0  # over-approximation, not the true [-0.25, 0]


def test_function_evaluate_bound_uses_atol_for_zero_sensitive_operations():
    x = Function(Linear(terms={1: 1.0}))
    point = {1: Bound(1e-8, 1e-8)}

    coarse_signum = x.signum().evaluate_bound(point, atol=1e-6)
    assert coarse_signum == Bound(0.0, 0.0)

    fine_signum = x.signum().evaluate_bound(point, atol=1e-12)
    assert fine_signum == Bound(1.0, 1.0)

    reciprocal = Function(1.0) / x
    with pytest.raises(RuntimeError, match="denominator may be classified as zero"):
        reciprocal.evaluate_bound(point, atol=1e-6)

    fine_reciprocal = reciprocal.evaluate_bound(point, atol=1e-12)
    assert fine_reciprocal.lower <= 1e8 <= fine_reciprocal.upper
