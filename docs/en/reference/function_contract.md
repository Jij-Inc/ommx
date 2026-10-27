# Function representation and numerical-evaluation contracts

This Reference separates representation and numerical conditions from the [Function Guide](../user_guide/function.md) for Python SDK **3.0.0b7**. See {class}`~ommx.Function` for operation arguments and return values. The [Function Message from the same release](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/proto/ommx/v1/function.proto) defines the wire representation; the [same release's Rust implementation](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/src/function/evaluate.rs) is the evaluation reference.

## Polynomials and composed expressions

Function retains constant, linear, quadratic, and polynomial expressions, and expressions composed using scalar operations. Polynomial addition, subtraction, multiplication, and negation normalize to polynomials. Terms that actually become zero through cancellation or floating-point underflow are removed, but finite nonzero coefficients are not dropped based on evaluation's `atol`.

Division on `Function` produces a composed expression even when the divisor is constant. Integer powers are also not automatically expanded, even with nonnegative exponents, except for identity operations or constant folding. Use, for example, `g * g` rather than `g ** 2` when expanded polynomial representation is required.

For composed expressions, `degree()` and `num_terms()` return `None`; polynomial coefficient attributes such as `terms` raise `TypeError`. `almost_equal()` compares polynomial coefficients or composed-expression structure, not global equivalence of arbitrary expressions.

## Operation order and wire representation

Composed expressions retain parentheses and operation order. Operands are evaluated left to right, combining two values for each addition or multiplication. For example, `(abs(a) + abs(b)) + abs(c)` and `abs(a) + (abs(b) + abs(c))` are not automatically reordered identically. Overflow and undefined operations also depend on this order.

Messages store composed expressions as flat reverse Polish notation (RPN) instruction sequences rather than recursively nested trees. The Function Message linked above defines instructions and wire structure. Building expressions in Python does not require constructing instructions directly.

## Integer powers and undefined inputs

`fx ** n` and `fx.powi(n)` are the same integer-power operation. The exponent is a signed 32-bit integer. Floating-point or function-valued exponents and reverse powers such as `2 ** fx` are unsupported.

Explicit evaluation treats $|v|\leq\mathtt{atol}$ as zero, including the boundary. Omitting `atol` uses that evaluation operation's default.

| Operation | Numerical condition or result |
|---|---|
| `signum(v)` | 0 when $|v|\leq\mathtt{atol}$; −1 below that interval and 1 above |
| Division | Undefined when the denominator's absolute value is at most `atol` |
| Negative integer power | Undefined when the base's absolute value is at most `atol` |
| Zeroth power | `0 ** 0` is also 1 |
| Negative base | Supported because the exponent is an integer |

Undefined operations and nonfinite intermediate results raise `ValueError` during Python point evaluation. Missing values for required variable IDs also prevent evaluation.

Expressions containing division or negative integer powers are not necessarily defined at every input. Simplification does not erase their domains: for example, `0 * (1 / fx)` is still undefined when $|\mathtt{fx}|\leq\mathtt{atol}$.

## Partial evaluation and tolerances

Construction, substitution, and partial evaluation do not prematurely settle a later `atol` using an implicit default. Zero-sensitive `Signum`, `Div`, and negative `Powi` operations remain in the expression; subsequent evaluation's `atol` determines results or domains. They can remain composed expressions even after partial evaluation replaces all variables.

Partial evaluation of `Linear`, `Quadratic`, and `Polynomial` retains the original Python type. Composed Functions retain structure depending on unassigned variables. Closed subexpressions are folded to constants when their results do not depend on `atol`.

{meth}`~ommx.Function.evaluate_bound` also uses the specified `atol` for zero-sensitive operations. Use the same tolerance when point-evaluating the expression. The resulting interval encloses the actual range but need not be tight, because it does not exploit every relationship between terms sharing variables. See the method's API Reference for interval-construction and undefined-operation errors.

These Function numerical conditions differ from Instance rules for rounding discrete candidate values and checking variable domains or constraint satisfaction. Use [Solution's evaluation model](../user_guide/solution.md) to check a complete candidate.
