# Expressions: `Linear`, `Quadratic`, `Polynomial`, and `Function`

These types represent mathematical expressions in optimization problems:

- **[`Linear`](crate::Linear)**: Up to degree 1 polynomials (linear terms + constant)
- **[`Quadratic`](crate::Quadratic)**: Up to degree 2 polynomials (may contain only linear terms, no quadratic terms required)
- **[`Polynomial`](crate::Polynomial)**: Polynomials of any finite degree
- **[`Function`](crate::Function)**: Either a compact polynomial or a composed scalar expression such as an absolute value, minimum, division, or signed 32-bit integer power

Use the convenience macros [`linear!`](crate::linear), [`quadratic!`](crate::quadratic), [`coeff!`](crate::coeff), and [`monomial!`](crate::monomial) for easy expression building.

```rust
use ommx::{Linear, Quadratic, Function, linear, quadratic, coeff};

// Linear expressions: 2*x1 + 3*x2 + 5 (fixed degree 1)
let linear_expr = (coeff!(2.0) * linear!(1))?;
let linear_expr = (linear_expr + (coeff!(3.0) * linear!(2))?)?;
let linear_expr = (linear_expr + coeff!(5.0))?;

// Quadratic expressions: x1*x2 + 2*x1 + 1 (up to degree 2)
let quad_expr = (coeff!(1.0) * quadratic!(1, 2))?;
let quad_expr = (quad_expr + (coeff!(2.0) * quadratic!(1))?)?;
let quad_expr = (quad_expr + coeff!(1.0))?;
assert_eq!(quad_expr.degree(), 2);

// Quadratic with only linear terms (no quadratic terms): 3*x1 + 2
let linear_only_quad = ((coeff!(3.0) * quadratic!(1))? + coeff!(2.0))?;
assert_eq!(linear_only_quad.degree(), 1);

// Compact polynomial Functions report their polynomial degree.
let linear_func = Function::from(linear_expr);  // Degree 1
assert_eq!(linear_func.degree(), Some(1.into()));
let quad_func = Function::from(quad_expr);      // Degree 2
assert_eq!(quad_func.degree(), Some(2.into()));

// Composed Functions are not classified by polynomial degree.
let absolute = linear_func.clone().abs();
assert_eq!(absolute.degree(), None);
let squared = linear_func.powi(2);
assert_eq!(squared.degree(), None);
# Ok::<(), ommx::CoefficientError>(())
```

Apart from identity and constant folding, `Function::powi` preserves an
explicit composed integer-power expression even for a non-negative exponent.
Use explicit multiplication when a compact expanded polynomial is required by
a polynomial-only solver or exporter.

A composed [`Function`](crate::Function) uses a single
[`Function::Expression`](crate::Function::Expression) variant backed by a
validated, flat reverse-Polish expression program. Every associative instruction
combines exactly two stack values, preserving the grouping and evaluation order
of each operation. Callers construct these programs through `Function` methods
and arithmetic operators rather than assembling instructions directly.

See also [`PolynomialBase`](crate::PolynomialBase) which is a base for [`Linear`](crate::Linear), [`Quadratic`](crate::Quadratic), and [`Polynomial`](crate::Polynomial).
