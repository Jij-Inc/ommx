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

# ommx.v1.Function

In mathematical optimization, functions are used to express objective functions and constraints. Specifically, OMMX handles polynomials and provides the following data structures in OMMX Message to represent polynomials.

| Data Structure | Description |
| --- | --- |
| [ommx.v1.Linear](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Linear) | Linear function. Holds pairs of variable IDs and their coefficients |
| [ommx.v1.Quadratic](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Quadratic) | Quadratic function. Holds pairs of variable ID pairs and their coefficients |
| [ommx.v1.Polynomial](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Polynomial) | Polynomial. Holds pairs of variable ID combinations and their coefficients |
| [ommx.v1.Function](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/v1/index.html#ommx.v1.Function) | One of the above or a constant |


## Creating ommx.v1.Function
In the Python SDK, there are two main approachs to create these data structures. The first approach is to directly call the constructors of each data structure. For example, you can create `ommx.v1.Linear` as follows.

```{code-cell} ipython3
from ommx.v1 import Linear

linear = Linear(terms={1: 1.0, 2: 2.0}, constant=3.0)
print(linear)
```

In this way, decision variables are identified by IDs and coefficients are represented by real numbers. To access coefficients and constant values, use the `terms`, `linear_terms` and `constant_term` properties.

```{code-cell} ipython3
print(f"{linear.terms=}")
print(f"{linear.linear_terms=}")
print(f"{linear.constant_term=}")
```

Another approach is to create from `ommx.v1.DecisionVariable`. `ommx.v1.DecisionVariable` is a data structure that only holds the ID of the decision variable. When creating polynomials such as `ommx.v1.Linear`, you can first create decision variables using `ommx.v1.DecisionVariable` and then use them to create polynomials.

```{code-cell} ipython3
from ommx.v1 import DecisionVariable

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

`Linear`, `Quadratic`, and `Polynomial` each have their own unique data storage methods, so they are separate Messages. However, since any of them can be used as objective functions or constraints, a Message called `Function` is provided, which can be any of the above or a constant.

```{code-cell} ipython3
from ommx.v1 import Function

# Constant
print(Function(1.0))
# Linear
print(Function(linear))
# Quadratic
print(Function(q))
# Polynomial
print(Function(p))
```

## Substitution and Partial Evaluation of Decision Variables

`Function` and other polynomials have an `evaluate` method that substitutes values for decision variables. For example, substituting $x_1 = 1$ and $x_2 = 0$ into the linear function $x_1 + 2x_2 + 3$ created above results in $1 + 2 \times 0 + 3 = 4$.

```{code-cell} ipython3
value= linear.evaluate({1: 1, 2: 0})
print(f"{value=}")
```

The argument supports the format `dict[int, float]` and `ommx.v1.State`. `evaluate` returns an error if the necessary decision variable IDs are missing.

```{code-cell} ipython3
try:
    linear.evaluate({1: 1})
except RuntimeError as e:
    print(f"Error: {e}")
```

If you want to substitute values for only some of the decision variables, use the `partial_evaluate` method.

```{code-cell} ipython3
linear2= linear.partial_evaluate({1: 1})
print(f"{linear2=}")
```

The result of partial evaluation is a polynomial, so it is returned in the same type as the original polynomial.

+++

## Comparison of Coefficients

`Function` and other polynomial types have an `almost_equal` function. This function determines whether the coefficients of the polynomial match within a specified error. For example, to confirm that $ (x + 1)^2 = x^2 + 2x + 1 $, write as follows

```{code-cell} ipython3
xx = (x + 1) * (x + 1)
xx.almost_equal(x * x + 2 * x + 1)
```
