# Decision Variables: `Bound`, `Kind`, and `DecisionVariable`

Decision variables define the unknowns in optimization problems. Each variable has a [`Kind`](crate::Kind)
(continuous, binary, integer, etc.) and [`Bound`](crate::Bound) (lower/upper limits).

```rust
use ommx::{DecisionVariable, Kind, Bound, ATol};

// Binary decision variable row
let binary_var = DecisionVariable::binary();
assert_eq!(binary_var.kind(), Kind::Binary);
assert_eq!(binary_var.bound(), Bound::new(0.0, 1.0)?); // Default binary bound is [0, 1]

// Integer variable with bound [0, 3]
let integer_var = DecisionVariable::integer()
    .with_bound(Bound::new(0.0, 3.0)?, ATol::default())?;
assert_eq!(integer_var.kind(), Kind::Integer);
assert_eq!(integer_var.bound(), Bound::new(0.0, 3.0)?);

// Continuous decision variable row
let continuous_var = DecisionVariable::continuous();
assert_eq!(continuous_var.kind(), Kind::Continuous);
assert_eq!(continuous_var.bound(), Bound::unbounded()); // Default is unbounded (-inf, inf)

// Finite-domain variable with an exact enumerated feasible set
let finite_var = DecisionVariable::new_finite_domain(vec![1.0, 0.1, 0.5])?;
assert_eq!(finite_var.kind(), Kind::FiniteDomain);
assert_eq!(finite_var.finite_domain().unwrap().values(), &[0.1, 0.5, 1.0]);
assert_eq!(finite_var.bound(), Bound::new(0.1, 1.0)?); // Derived convex hull
# Ok::<(), Box<dyn std::error::Error>>(())
```

A [`FiniteDomain`](crate::FiniteDomain) is the exact feasible set, not a
discretization of a continuous interval. Its values must be non-empty, finite,
and unique.
