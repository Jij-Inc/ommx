---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# ommx.Function

In mathematical optimization, functions are used to express objective functions and constraints. OMMX stores polynomials compactly and can compose them with scalar operations such as absolute value, minimum, division, and integer power.

| Data Structure | Description |
| --- | --- |
| {class}`~ommx.Linear` | Linear function. Holds pairs of variable IDs and their coefficients |
| {class}`~ommx.Quadratic` | Quadratic function. Holds pairs of variable ID pairs and their coefficients |
| {class}`~ommx.Polynomial` | Polynomial. Holds pairs of variable ID combinations and their coefficients |
| {class}`~ommx.Function` | A polynomial or a composed scalar expression |


## Creating ommx.Function
In the Python SDK, there are two main approaches to create these data structures. The first approach is to directly call the constructors of each data structure. For example, you can create `ommx.Linear` as follows.

```{code-cell} ipython3
from ommx import Linear

linear = Linear(terms={1: 1.0, 2: 2.0}, constant=3.0)
print(linear)
```

In this way, decision variables are identified by IDs and coefficients are represented by real numbers. To access coefficients and constant values, use the `terms`, `linear_terms` and `constant_term` properties.

```{code-cell} ipython3
print(f"{linear.terms=}")
print(f"{linear.linear_terms=}")
print(f"{linear.constant_term=}")
```

Another approach is to create from `ommx.DecisionVariable`. `ommx.DecisionVariable` is a data structure that only holds the ID of the decision variable. When creating polynomials such as `ommx.Linear`, you can first create decision variables using `ommx.DecisionVariable` and then use them to create polynomials.

```{code-cell} ipython3
from ommx import DecisionVariable

x = DecisionVariable.binary(1, name="x")
y = DecisionVariable.binary(2, name="y")

linear = x + 2.0 * y + 3.0
print(linear)
```

Note that the polynomial data type retains only the ID of the decision variable and does not store additional information. In the above example, information passed to `DecisionVariable.binary` such as `x` and `y` is not carried over to `Linear`. This second method can create polynomials of any degree.

```{code-cell} ipython3
q = x * x + x * y + y * y
print(q)
```

```{code-cell} ipython3
p = x * x * x + y * y
print(p)
```

`Linear`, `Quadratic`, and `Polynomial` each have their own unique data storage methods, so they are separate Messages. Since any of them can be used as objective functions or constraints, `Function` provides one common representation and also owns composed scalar expressions.

```{code-cell} ipython3
from ommx import Function

# Constant
print(Function(1.0))
# Linear
print(Function(linear))
# Quadratic
print(Function(q))
# Polynomial
print(Function(p))
```

## Composing Scalar Functions

Convert an arithmetic expression to `Function` before applying the composed operations. Python's `abs` and arithmetic operators build composed expressions; pointwise minimum and maximum use the named `minimum` and `maximum` methods.

```{code-cell} ipython3
fx = Function(x)
fy = Function(y)

absolute = abs(fx - 3)
sign = (fx - 1).signum()
minimum = fx.minimum(fy)
maximum = fx.maximum(fy)
quotient = fx / (fy + 1)
power = fx**2
same_power = fx.powi(2)

print(absolute)
print(minimum)
print(quotient)
print(power)
print(same_power)
```

The built-in Python functions `min(fx, fy)` and `max(fx, fy)` perform comparisons and therefore do not construct pointwise expressions. Use `fx.minimum(fy)` and `fx.maximum(fy)` instead.

`fx**n` and `fx.powi(n)` are equivalent, and `n` must fit in a signed 32-bit integer. Floating-point or function-valued exponents and reverse exponentiation such as `2**fx` are not supported.

Within a composed expression, operands are encountered from left to right and every associative application still combines exactly two values. Both `(abs(a) + abs(b)) + abs(c)` and `abs(a) + (abs(b) + abs(c))` therefore retain their grouping and operation order. Overflow and undefined-operation errors occur in that represented order. The protobuf wire format stores the composition as a flat reverse Polish notation (RPN) instruction sequence rather than a recursively nested tree. Exact addition, subtraction, multiplication, and negation containing only polynomials continue to normalize to the compact polynomial representation. Division of a `Function` remains a composed expression even when the divisor is a constant or coefficient, because whether that divisor is treated as zero depends on the evaluation tolerance.

Composed functions follow real-valued evaluation semantics. At an explicit evaluation entry point, the `atol` argument defines zero as $|v| \leq \mathtt{atol}$ (including the boundary):

- `signum(v)` is `0` when $|v| \leq \mathtt{atol}$, `-1` below that interval, and `1` above it.
- Division is undefined when the absolute value of the denominator is at most `atol`.
- Raising a value with absolute value at most `atol` to a negative integer power is undefined; `0**0` is defined as `1`.
- Negative bases are supported because every exponent is an integer.
- Undefined operations and non-finite intermediate results raise `ValueError` in Python.

If `atol` is omitted, that evaluation call uses the configured default tolerance. Expression construction, substitution, and partial evaluation do not use a default tolerance to decide these zero-sensitive operations. Their `Signum`, `Div`, and negative `Powi` nodes remain in the RPN expression so a later evaluation call can apply its own `atol`.

Division and negative integer powers can make a `Function` partial. Algebraic simplification preserves that domain: for example, `0 * (1 / fx)` is still undefined wherever $|\mathtt{fx}| \leq \mathtt{atol}$.

```{code-cell} ipython3
try:
    (1 / fx).evaluate({1: 0}, atol=1e-6)
except ValueError as e:
    print(f"Error: {e}")
```

Polynomial metadata is available only when the `Function` uses the compact polynomial representation. `degree()` and `num_terms()` return `None` for composed expressions, while coefficient properties such as `terms` raise `TypeError`. Apart from identity and constant folding, an integer-power expression remains composed even when its exponent is non-negative; use explicit multiplication when you need a compact expanded polynomial. Solver adapters also reject a composed expression unless their declared input class supports it.

Compact polynomial normalization is exact: it removes an actual zero produced by cancellation or floating-point underflow, but retains every finite nonzero coefficient regardless of magnitude. It does not use `atol` to prune small terms. OMMX does not currently perform approximate cleanup implicitly; such cleanup would require a separate, explicit API rather than being part of expression construction or evaluation.

## Substitution and Partial Evaluation of Decision Variables

`Function` and the polynomial types have an `evaluate` method that substitutes values for decision variables. For example, substituting $x_1 = 1$ and $x_2 = 0$ into the linear function $x_1 + 2x_2 + 3$ created above results in $1 + 2 \times 0 + 3 = 4$.

```{code-cell} ipython3
value= linear.evaluate({1: 1, 2: 0})
print(f"{value=}")
```

The argument supports the format `dict[int, float]` and `ommx.State`. `evaluate` returns an error if the necessary decision variable IDs are missing.

```{code-cell} ipython3
try:
    linear.evaluate({1: 1})
except ValueError as e:
    print(f"Error: {e}")
```

If you want to substitute values for only some of the decision variables, use the `partial_evaluate` method.

```{code-cell} ipython3
linear2= linear.partial_evaluate({1: 1})
print(f"{linear2=}")
```

`Linear`, `Quadratic`, and `Polynomial` partial evaluation keeps the original Python type. For a composed `Function`, the expression structure that still depends on unassigned variables is preserved. A closed subexpression may be folded only when its result is independent of `atol`. Zero-sensitive `Signum`, `Div`, and negative `Powi` subexpressions remain composed even after all their variables have been assigned, so the `atol` of a later `evaluate` call still determines their result or domain error.

+++

## Comparison of Coefficients

`Function` and the polynomial types have an `almost_equal` function. For polynomial functions, it determines whether the coefficients match within a specified error. For composed functions, it compares matching expression structures; it is not a proof of global mathematical equivalence. For example, to confirm that $ (x + 1)^2 = x^2 + 2x + 1 $, write as follows

```{code-cell} ipython3
xx = (x + 1) * (x + 1)
xx.almost_equal(x * x + 2 * x + 1)
```
