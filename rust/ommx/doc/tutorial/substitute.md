# The `Substitute` trait

The [`Substitute`](crate::Substitute) trait enables symbolic substitution of variables with expressions,
allowing for problem transformation and preprocessing.

```rust
use ommx::{Substitute, Function, Linear, linear, coeff, assign, ATol};
use approx::assert_abs_diff_eq;

// Original expression: 2*x1 + 1
let expr = coeff!(2.0) * linear!(1) + Linear::one();

// Substitute x1 = 0.5*x2 + 1
let assignments = assign! {
    1 <- coeff!(0.5) * linear!(2) + Linear::one()
};

let substituted = expr.substitute_acyclic(&assignments)?;
assert_abs_diff_eq!(
  substituted,
  Function::from(linear!(2) + coeff!(3.0))  // Result: 2*(0.5*x2 + 1) + 1 = x2 + 3
);
# Ok::<(), Box<dyn std::error::Error>>(())
```
