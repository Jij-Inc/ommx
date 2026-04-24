# Expressions: `Linear`, `Quadratic`, `Polynomial`, and `Function`

These types represent mathematical expressions in optimization problems with different degree characteristics:

- **[`Linear`](crate::Linear)**: Up to degree 1 polynomials (linear terms + constant)
- **[`Quadratic`](crate::Quadratic)**: Up to degree 2 polynomials (may contain only linear terms, no quadratic terms required)
- **[`Function`](crate::Function)**: Dynamic degree handling, can represent any polynomial degree at runtime

Use the convenience macros [`linear!`](crate::linear), [`quadratic!`](crate::quadratic), [`coeff!`](crate::coeff), and [`monomial!`](crate::monomial) for easy expression building.

```rust
use ommx::{Linear, Quadratic, Function, linear, quadratic, coeff};

// Linear expressions: 2*x1 + 3*x2 + 5 (fixed degree 1)
let linear_expr = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

// Quadratic expressions: x1*x2 + 2*x1 + 1 (up to degree 2)
let quad_expr = coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);
assert_eq!(quad_expr.degree(), 2);

// Quadratic with only linear terms (no quadratic terms): 3*x1 + 2
let linear_only_quad = coeff!(3.0) * quadratic!(1) + coeff!(2.0);
assert_eq!(linear_only_quad.degree(), 1);

// Functions can dynamically handle any degree
let linear_func = Function::from(linear_expr);  // Degree 1
assert_eq!(linear_func.degree(), 1);
let quad_func = Function::from(quad_expr);      // Degree 2
assert_eq!(quad_func.degree(), 2);
```

See also [`PolynomialBase`](crate::PolynomialBase) which is a base for [`Linear`](crate::Linear), [`Quadratic`](crate::Quadratic), and [`Polynomial`](crate::Polynomial).
