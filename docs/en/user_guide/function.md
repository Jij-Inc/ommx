---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# Function: objective and constraint expressions

A {class}`~ommx.Function` is an expression mapping variable values to a number. It is used for objectives and ordinary-constraint bodies. **Constructing an expression, evaluating it, and having a solver accept a problem containing it are different things.** This page covers construction, evaluation, and representation choices.

## Build an expression and evaluate it

Apply arithmetic operations to decision variables to build expressions. Here, $f(x,y)=x+2y+3$.

```{code-cell} ipython3
from ommx import Function, Instance

instance = Instance.minimize()
x = instance.new_binary("x")
y = instance.new_binary("y")
f = Function(x + 2 * y + 3)

assert f.evaluate({x.id: 1, y.id: 0}) == 4.0
instance.objective = f
instance.add_constraint(x + y <= 1)
```

{meth}`~ommx.Function.evaluate` accepts a dictionary of variable IDs and values, or a {class}`~ommx.State`. Evaluation requires values for every ID needed by the expression. A standalone Function references IDs, without storing variable names, kinds, or bounds. To check domains and constraints as well, use {meth}`~ommx.Instance.evaluate` rather than Function evaluation.

Use {meth}`~ommx.Function.partial_evaluate` to assign only some variables. It returns an expression in the remaining variables without changing the original.

```{code-cell} ipython3
remaining = f.partial_evaluate({x.id: 1})
assert remaining.evaluate({y.id: 0}) == 4.0
assert remaining.evaluate({y.id: 1}) == 6.0
```

## Construct polynomials

Addition, subtraction, and multiplication can build quadratic and higher-degree polynomials. There are also types for construction directly from variable IDs and coefficients.

| Type | Expression |
|---|---|
| {class}`~ommx.Linear` | Linear expression |
| {class}`~ommx.Quadratic` | Polynomial of degree at most two |
| {class}`~ommx.Polynomial` | Polynomial of arbitrary degree |
| {class}`~ommx.Function` | These polynomials or expressions composed using the scalar operations below |

```{code-cell} ipython3
from ommx import Linear

by_coefficients = Linear(terms={x.id: 1, y.id: 2}, constant=3)
assert Function(by_coefficients).almost_equal(f)

quadratic = Function(x * x + 2 * x * y + y * y)
assert quadratic.degree() == 2
assert quadratic.evaluate({x.id: 1, y.id: 1}) == 4.0
```

Inspect degree and coefficients with `degree()`, `terms`, `linear_terms`, `quadratic_terms`, `constant_term`, and related attributes. See each type's API Reference for the attributes it provides.

For polynomials, {meth}`~ommx.Function.almost_equal` compares coefficients within a tolerance. For general expressions, it compares expression structure; it does not determine whether arbitrary expressions agree at every point.

## Combine scalar operations

Function also supports absolute value, sign, pointwise minimum/maximum, division, and integer powers. These operations construct expressions; they do not assign variable values.

```{code-cell} ipython3
fx = Function(x)
fy = Function(y)

absolute = abs(fx - 3)
minimum = fx.minimum(fy)
quotient = fx / (fy + 1)
power = fx**2

assert absolute.evaluate({x.id: 1}) == 2.0
assert minimum.evaluate({x.id: 1, y.id: 0}) == 0.0
assert quotient.evaluate({x.id: 1, y.id: 1}) == 0.5
assert power.evaluate({x.id: 1}) == 1.0
```

Use `minimum()` and `maximum()` for pointwise minimum and maximum. Python's built-in `min()` and `max()` are not substitutes. Use `**` or `powi()` for integer powers.

Division and negative integer powers may be undefined for some inputs. Evaluation's `atol` also determines when a denominator or similar value counts as zero, so changing the tolerance can change the result. See [Function contracts](../reference/function_contract.md) for operation domains, numerical conditions, and errors.

## Mathematical degree and retained representation differ

For solver use, check not only the mathematical expression but also the representation retained by Function. For example, `g ** 2` on a Function does not automatically become an expanded polynomial. Use `g * g` when you need a polynomial square.

```{code-cell} ipython3
g = Function(x + y)
composed_square = g**2
polynomial_square = g * g

assert composed_square.degree() is None
assert polynomial_square.degree() == 2
assert polynomial_square.almost_equal(quadratic)
```

These expressions can calculate the same values but differ in membership of a class requiring polynomial representation. A composed expression has `degree() is None`, and its coefficient attributes cannot be used as for a polynomial.

The SDK's InstanceClass in this Book requires polynomial representation for objectives and constraint bodies. An evaluable Function is therefore not necessarily accepted as input by the documented Adapters. [InstanceClass and input conditions](./instance_class.md) explains membership in the required representation. Preparation does not automatically expand arbitrary composed expressions into polynomials.

## Further references

[Function contracts](../reference/function_contract.md) covers numerical zero tests, operation order, operations retained during partial evaluation, and coefficient normalization. Use {meth}`~ommx.Function.evaluate_bound` for interval evaluation, but its interval need not equal the exact minimum and maximum. See each method's API Reference for arguments, return values, and interval-evaluation conditions.

For cross-language storage and exchange, see the [Message specification](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto) for wire representations such as RPN instructions. These representation details are not prerequisites for building an objective and evaluating a candidate in Python.
