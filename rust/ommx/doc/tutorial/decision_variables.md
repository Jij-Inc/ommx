# Decision Variables: `Bound`, `Kind`, and `DecisionVariable`

Decision variables define the unknowns in optimization problems. Each variable has a [`Kind`](crate::Kind)
(continuous, binary, integer, etc.) and [`Bound`](crate::Bound) (lower/upper limits).

```rust
use ommx::{DecisionVariable, Kind, Bound, VariableID, ATol};

// Binary decision variable with ID 1
let binary_var = DecisionVariable::binary(VariableID::from(1));
assert_eq!(binary_var.kind(), Kind::Binary);
assert_eq!(binary_var.bound(), Bound::new(0.0, 1.0)?); // Default binary bound is [0, 1]

// Integer variable with bound [0, 3]
let integer_var = DecisionVariable::integer(VariableID::from(2))
    .with_bound(Bound::new(0.0, 3.0)?, ATol::default())?;
assert_eq!(integer_var.kind(), Kind::Integer);
assert_eq!(integer_var.bound(), Bound::new(0.0, 3.0)?);

// Continuous variable with ID 3
let continuous_var = DecisionVariable::continuous(VariableID::from(3));
assert_eq!(continuous_var.kind(), Kind::Continuous);
assert_eq!(continuous_var.bound(), Bound::unbounded()); // Default is unbounded (-inf, inf)
# Ok::<(), Box<dyn std::error::Error>>(())
```
