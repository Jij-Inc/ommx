# The `Evaluate` trait

The [`Evaluate`](crate::Evaluate) trait allows evaluation of expressions and functions given variable assignments.
This is essential for solution verification and constraint checking.

```rust
use ommx::{Evaluate, Function, linear, coeff, ATol};
use ommx::v1::State;
use std::collections::HashMap;

// Create a function: 2*x1 + 3*x2
let func = Function::from(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2));

// Create variable assignments
let state = State::from(HashMap::from([(1, 4.0), (2, 5.0)]));

// Evaluate: 2*4 + 3*5 = 23
let result = func.evaluate(&state, ATol::default())?;
assert_eq!(result, 23.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```
