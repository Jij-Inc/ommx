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

OMMX supports four first-class constraint types beyond standard
`f(x) = 0` / `f(x) <= 0`:

- **[`Constraint`](crate::Constraint)** — standard equality / inequality
  constraints on a [`Function`](crate::Function).
- **[`IndicatorConstraint`](crate::IndicatorConstraint)** —
  `indicator_variable = 1 → f(x) {=,<=} 0`, keyed by
  [`IndicatorConstraintID`](crate::IndicatorConstraintID).
- **[`OneHotConstraint`](crate::OneHotConstraint)** — exactly one of a
  set of binary variables is 1, keyed by
  [`OneHotConstraintID`](crate::OneHotConstraintID).
- **[`Sos1Constraint`](crate::Sos1Constraint)** — at most one of a set
  of variables is non-zero, keyed by
  [`Sos1ConstraintID`](crate::Sos1ConstraintID).

Each constraint type also has its own independent ID type to prevent
accidental cross-type lookups, and each ID lives on the enclosing
[`BTreeMap`](std::collections::BTreeMap) key rather than on the
constraint struct.

### Stage parameter

Each constraint type follows the **Stage pattern** — parameterized by
lifecycle phase, with three inhabited stages:

- `Created` — the constraint as defined in the problem, carrying its
  [`Function`](crate::Function).
- `Evaluated` — the result of evaluating the constraint against a
  single [`State`](crate::v1::State); carries `evaluated_value`,
  `feasible`, `dual_variable`, and the set of variable IDs it used.
- `Sampled` — the result of evaluating against a
  [`Sampled<State>`](crate::Sampled); carries per-sample evaluated
  values and feasibility.

Each constraint type implements the
[`ConstraintType`](crate::ConstraintType) trait, which maps all three
stages as associated types — a defunctionalization of `Stage → Type`
since Rust lacks HKTs. "Removed" is **not** a stage: removal is
collection-level metadata tracked as a
[`RemovedReason`](crate::RemovedReason) paired with the original
`Created` constraint.

### Collections

Three generic collection wrappers handle constraints uniformly:

- [`ConstraintCollection`](crate::ConstraintCollection) —
  active + removed, used in [`Instance`](crate::Instance). Removed
  constraints are stored as `(T::Created, RemovedReason)` tuples.
- [`EvaluatedCollection`](crate::EvaluatedCollection) — evaluation
  results, used in [`Solution`](crate::Solution).
- [`SampledCollection`](crate::SampledCollection) — sampled results,
  used in [`SampleSet`](crate::SampleSet).

To add a new constraint type, see the docs on
[`ConstraintType`](crate::ConstraintType).
