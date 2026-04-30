"""Arithmetic tests for AttachedDecisionVariable.

These cover the `ToFunction` extension and the polymorphic operators on
DecisionVariable / AttachedDecisionVariable / Linear / Quadratic / Polynomial
that now extract `AttachedDecisionVariable` via the shared `FunctionInput`
enum. The fixture builds an Instance once with a few decision variables and
each test exercises one operator pair.
"""

from __future__ import annotations

import pytest

from ommx.v1 import (
    AttachedDecisionVariable,
    Constraint,
    DecisionVariable,
    Function,
    Instance,
    Linear,
    Polynomial,
    Quadratic,
)


@pytest.fixture
def instance() -> Instance:
    return Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=Function.from_linear(Linear.constant(0.0)),
        decision_variables=[
            DecisionVariable.integer(0, lower=0, upper=10, name="x"),
            DecisionVariable.integer(1, lower=0, upper=10, name="y"),
        ],
        constraints={},
    )


def _vars(
    instance: Instance,
) -> tuple[AttachedDecisionVariable, AttachedDecisionVariable]:
    """Two attached handles, sanity-checked."""
    variables = instance.decision_variables
    assert all(isinstance(v, AttachedDecisionVariable) for v in variables)
    return variables[0], variables[1]


def _terms(linear: Linear) -> dict[int, float]:
    return dict(linear.linear_terms)


def _constant(linear: Linear) -> float:
    return linear.constant_term


# --- AttachedDecisionVariable on the LHS ---


def test_neg_returns_linear_with_negative_coefficient(instance: Instance):
    x, _ = _vars(instance)
    result = -x
    assert isinstance(result, Linear)
    assert _terms(result) == {0: -1.0}


def test_attached_plus_attached_returns_linear(instance: Instance):
    x, y = _vars(instance)
    result = x + y
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0, 1: 1.0}


def test_attached_plus_scalar_returns_linear(instance: Instance):
    x, _ = _vars(instance)
    result = x + 3
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0}
    assert _constant(result) == 3.0


def test_attached_plus_scalar_zero_returns_linear_unchanged(instance: Instance):
    x, _ = _vars(instance)
    result = x + 0
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0}
    assert _constant(result) == 0.0


def test_attached_plus_dv_snapshot_returns_linear(instance: Instance):
    """`AttachedDV + DV` should also work — DV-as-snapshot can be created via
    `detach()`, but a freshly constructed `DecisionVariable` should compose too."""
    x, _ = _vars(instance)
    z = DecisionVariable.integer(2, lower=0, upper=1)
    result = x + z
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0, 2: 1.0}


def test_attached_minus_attached_returns_linear(instance: Instance):
    x, y = _vars(instance)
    result = x - y
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0, 1: -1.0}


def test_attached_times_attached_returns_quadratic(instance: Instance):
    x, y = _vars(instance)
    result = x * y
    assert isinstance(result, Quadratic)
    assert dict(result.quadratic_terms) == {(0, 1): 1.0}


def test_attached_times_scalar_returns_linear(instance: Instance):
    x, _ = _vars(instance)
    result = x * 5
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 5.0}


def test_attached_times_scalar_zero_returns_zero_linear(instance: Instance):
    x, _ = _vars(instance)
    result = x * 0
    assert isinstance(result, Linear)
    assert _terms(result) == {}
    assert _constant(result) == 0.0


def test_attached_times_quadratic_returns_polynomial(instance: Instance):
    x, y = _vars(instance)
    quad = x * y  # Quadratic
    assert isinstance(quad, Quadratic)
    result = x * quad
    assert isinstance(result, Polynomial)


def test_scalar_minus_attached_uses_rsub(instance: Instance):
    x, _ = _vars(instance)
    result = 5 - x
    assert isinstance(result, Linear)
    assert _terms(result) == {0: -1.0}
    assert _constant(result) == 5.0


def test_scalar_times_attached_uses_rmul(instance: Instance):
    x, _ = _vars(instance)
    result = 3 * x
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 3.0}


# --- AttachedDecisionVariable on the RHS of other types ---


def test_linear_plus_attached_returns_linear(instance: Instance):
    x, y = _vars(instance)
    base = 2 * x  # Linear
    assert isinstance(base, Linear)
    result = base + y
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 2.0, 1: 1.0}


def test_quadratic_plus_attached_returns_quadratic(instance: Instance):
    x, y = _vars(instance)
    quad = x * y  # Quadratic
    result = quad + x
    assert isinstance(result, Quadratic)
    assert dict(result.quadratic_terms) == {(0, 1): 1.0}
    assert dict(result.linear_terms) == {0: 1.0}


def test_dv_plus_attached_returns_linear(instance: Instance):
    x, _ = _vars(instance)
    z = DecisionVariable.integer(2, lower=0, upper=1)
    result = z + x
    assert isinstance(result, Linear)
    assert _terms(result) == {0: 1.0, 2: 1.0}


# --- Comparison operators (return Constraint) ---


def test_attached_eq_creates_constraint(instance: Instance):
    x, _ = _vars(instance)
    constraint = x == 5
    assert isinstance(constraint, Constraint)


def test_attached_le_creates_constraint(instance: Instance):
    x, _ = _vars(instance)
    constraint = x <= 5
    assert isinstance(constraint, Constraint)


def test_attached_ge_creates_constraint(instance: Instance):
    x, _ = _vars(instance)
    constraint = x >= 5
    assert isinstance(constraint, Constraint)


# --- ToFunction extraction ---


def test_function_constructor_accepts_attached(instance: Instance):
    """`Function(att_dv)` should work via `ToFunction`."""
    x, _ = _vars(instance)
    f = Function(x)
    assert isinstance(f, Function)
    assert f.linear_terms == {0: 1.0}


def test_attached_plus_function_returns_function(instance: Instance):
    """When the rhs is an opaque `Function`, the result preserves the
    `Function` shape (matches the previous `__radd__`-fallback behavior)."""
    x, y = _vars(instance)
    base = Function(2 * y + 1)  # Function
    result = x + base
    assert isinstance(result, Function)
