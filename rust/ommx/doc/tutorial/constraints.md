# Constraints

Constraints define the feasible region of optimization problems. Constraints can be equality
or inequality types, and can be temporarily removed while preserving their definition.

```rust
use ommx::{Constraint, Function, Linear, linear, coeff};

// Create constraints: x1 + x2 <= 10 (as x1 + x2 - 10 <= 0)
let constraint_expr = coeff!(1.0) * linear!(1) + coeff!(1.0) * linear!(2) + Linear::from(coeff!(-10.0));
let constraint = Constraint::less_than_or_equal_to_zero(Function::from(constraint_expr));

// Equality constraint: x1 - x2 = 0 (as f(x) = 0)
let eq_expr = coeff!(1.0) * linear!(1) - coeff!(1.0) * linear!(2);
let eq_constraint = Constraint::equal_to_zero(Function::from(eq_expr));
```

## Constraint Type System

OMMX supports multiple constraint types beyond standard `f(x) = 0` / `f(x) <= 0`:

- **[`Constraint`](crate::Constraint)**: Standard constraints
- **[`IndicatorConstraint`](crate::IndicatorConstraint)**: `indicator_variable = 1 → f(x) <= 0`

Each constraint type follows the **Stage pattern** — parameterized by lifecycle phase
(`Created`, `Removed`, `Evaluated`, `Sampled`) — and implements the
[`ConstraintType`](crate::ConstraintType) trait, which maps all four stages
as associated types (a defunctionalization of `Stage → Type` since Rust lacks HKTs).

Each constraint type also has its own independent ID type
([`ConstraintID`](crate::ConstraintID), [`IndicatorConstraintID`](crate::IndicatorConstraintID)) to prevent accidental cross-type lookups.

Three generic collection wrappers handle constraints uniformly:

- [`ConstraintCollection`](crate::ConstraintCollection): active + removed (used in [`Instance`](crate::Instance))
- [`EvaluatedCollection`](crate::EvaluatedCollection): evaluation results (used in [`Solution`](crate::Solution))
- [`SampledCollection`](crate::SampledCollection): sampled results (used in [`SampleSet`](crate::SampleSet))

To add a new constraint type, see the docs on [`ConstraintType`](crate::ConstraintType).
