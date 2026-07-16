import math
import sys

import pytest

import ommx


@pytest.mark.parametrize("value", [0.0, -1.0, math.nan])
def test_invalid_default_atol_raises_value_error(value: float) -> None:
    with pytest.raises(ValueError, match="ATol"):
        ommx.set_default_atol(value)


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.Linear({}, 0.0).almost_equal(ommx.Linear({}, 0.0), atol=0.0),
        lambda: ommx.Quadratic([], [], []).almost_equal(
            ommx.Quadratic([], [], []), atol=0.0
        ),
        lambda: ommx.Polynomial({}).almost_equal(ommx.Polynomial({}), atol=0.0),
        lambda: ommx.Function(0.0).almost_equal(ommx.Function(0.0), atol=0.0),
        lambda: ommx.Bound(0.0, 1.0).contains(0.5, atol=0.0),
        lambda: ommx.Linear({}, 0.0).evaluate(ommx.State({}), atol=0.0),
    ],
)
def test_invalid_operation_atol_raises_value_error(operation) -> None:
    with pytest.raises(ValueError, match="ATol must be positive"):
        operation()


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.Bound(2.0, 1.0),
        lambda: ommx.DecisionVariable.integer(0, lower=math.nan),
        lambda: ommx.DecisionVariable.continuous(0, lower=math.inf),
        lambda: ommx.DecisionVariable.semi_integer(0, lower=2.0, upper=1.0),
        lambda: ommx.DecisionVariable.semi_continuous(0, upper=-math.inf),
    ],
)
def test_invalid_bound_raises_value_error(operation) -> None:
    with pytest.raises(ValueError):
        operation()


@pytest.mark.parametrize(
    "operation",
    [
        lambda: ommx.Linear({0: math.inf}),
        lambda: ommx.Linear.constant(math.nan),
        lambda: ommx.Quadratic([0], [0], [math.inf]),
        lambda: ommx.Polynomial({(0,): math.nan}),
        lambda: ommx.Function.from_scalar(math.inf),
        lambda: ommx.Function.from_scalar(math.nan),
    ],
)
def test_invalid_coefficient_raises_value_error(operation) -> None:
    with pytest.raises(ValueError, match="Coefficient must"):
        operation()


def test_python_owned_shape_validation_stays_value_error() -> None:
    with pytest.raises(ValueError, match="Input vectors must have the same length"):
        ommx.Quadratic([0], [], [1.0])


def test_zero_coefficient_stays_a_normalized_success() -> None:
    assert ommx.Function.from_scalar(0.0).terms == {}
    assert ommx.Linear({0: 0.0}).terms() == {}


def test_non_finite_function_input_stays_type_error() -> None:
    x = ommx.DecisionVariable.binary(0)

    with pytest.raises(TypeError, match="unsupported operand type"):
        _ = x + math.inf


@pytest.mark.parametrize(
    ("lhs", "rhs", "terms"),
    [
        (
            lambda: ommx.Linear({0: 1.0, 2: sys.float_info.max}),
            lambda: ommx.Linear({0: 2.0, 2: sys.float_info.max}),
            lambda value: value.terms(),
        ),
        (
            lambda: ommx.Quadratic([0, 2], [0, 2], [1.0, sys.float_info.max]),
            lambda: ommx.Quadratic([0, 2], [0, 2], [2.0, sys.float_info.max]),
            lambda value: value.terms(),
        ),
        (
            lambda: ommx.Polynomial({(0,): 1.0, (2,): sys.float_info.max}),
            lambda: ommx.Polynomial({(0,): 2.0, (2,): sys.float_info.max}),
            lambda value: value.terms(),
        ),
        (
            lambda: ommx.Function(
                ommx.Polynomial({(0,): 1.0, (2,): sys.float_info.max})
            ),
            lambda: ommx.Function(
                ommx.Polynomial({(0,): 2.0, (2,): sys.float_info.max})
            ),
            lambda value: value.terms,
        ),
    ],
    ids=["linear", "quadratic", "polynomial", "function"],
)
def test_in_place_add_overflow_preserves_left_operand(lhs, rhs, terms) -> None:
    value = lhs()
    before = dict(terms(value))

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        value += rhs()

    assert terms(value) == before


def test_function_reduce_binary_power_overflow_raises_value_error() -> None:
    huge = sys.float_info.max
    function = ommx.Function(ommx.Polynomial({(0,): huge, (0, 0): huge}))
    before = dict(function.terms)

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        function.reduce_binary_power({0})

    assert function.terms == before


def test_instance_reduce_binary_power_overflow_raises_value_error() -> None:
    huge = sys.float_info.max
    instance = ommx.Instance.minimize()
    variable = instance.new_binary("x")
    instance.objective = ommx.Function(
        ommx.Polynomial({(variable.id,): huge, (variable.id, variable.id): huge})
    )
    before = instance.objective

    with pytest.raises(ValueError, match="Coefficient must be finite"):
        instance.reduce_binary_power()

    assert instance.objective.almost_equal(before)
