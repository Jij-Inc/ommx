# The `Substitute` trait

The [`Substitute`](crate::Substitute) trait enables symbolic substitution of variables with expressions,
allowing for problem transformation and preprocessing.

Substitution is an algebraic rewrite. It replaces occurrences of an assigned variable with the
assigned expression, but it does not automatically translate the assigned variable's domain into
constraints on that expression. If `x1` is binary and you substitute `x1 <- x2 + x3`, the rewrite
does not add `0 <= x2 + x3 <= 1`; if `x1` is integer, it does not add an integrality constraint on
`x2 + x3`.

This is intentional because some uses of substitution are not model-preserving transformations, and
some important uses provide their own validity proof. Binary encodings are the typical example:
the encoding is constructed so that the replacement expression already respects the original
variable's kind and bound. For a general substitution, the caller is responsible for preserving the
optimization problem when that is required, for example by adding a linking equality or explicit
bound constraints.

```rust
use ommx::{Substitute, Function, Linear, linear, coeff, assign, ATol};
use approx::assert_abs_diff_eq;

// Original expression: 2*x1 + 1
let expr = ((coeff!(2.0) * linear!(1))? + Linear::one())?;

// Substitute x1 = 0.5*x2 + 1
let assignments = assign! {
    1 <- (coeff!(0.5) * linear!(2)).unwrap() + Linear::one()
};

let substituted = expr.substitute_acyclic(&assignments)?;
assert_abs_diff_eq!(
  substituted,
  Function::from((linear!(2) + coeff!(3.0))?)  // Result: 2*(0.5*x2 + 1) + 1 = x2 + 3
);
# Ok::<(), Box<dyn std::error::Error>>(())
```
