# Indicator Constraint Design

## Overview

Indicator constraints express conditional constraints of the form:

```
binvar = 1 → f(x) <= 0  (or f(x) = 0)
```

When the binary indicator variable is 0, the constraint is unconditionally satisfied.

This is the first step toward a broader refactoring: making multiple constraint types first-class citizens in `Instance`, replacing the current `ConstraintHints` approach.

## Motivation

### Current limitations

Currently, OMMX `Constraint` can only represent unconditional constraints `f(x) = 0` or `f(x) <= 0`. Special constraint types like SOS1 are handled via `ConstraintHints`, which require:

1. Encoding the constraint using Big-M reformulation as regular constraints
2. Adding a `ConstraintHints` entry referencing those constraint IDs

This is lossy (solver-specific hints rather than semantic constraints) and fragile (hints reference constraint IDs that may become stale).

### Goal: multi-type constraint architecture

`Instance` will hold multiple typed constraint collections, each with its own data structure and semantics:

```
Instance
  ├── constraints                  (regular: f(x) = 0, f(x) <= 0)
  ├── indicator_constraints        (binary_var = 1 → f(x) <= 0)
  ├── disjunction_constraints      (C1 ∨ C2 ∨ ... ∨ Cn)
  ├── one_hot_constraints          (x1 + ... + xn = 1, all binary)
  ├── sos1_constraints             (at most one nonzero)
  └── ...
```

All constraint types share a common `ConstraintID` space. Each type has:
- Its own data structure and fields
- Its own `evaluate` / `partial_evaluate` / `required_ids` logic
- Adapter-specific conversion (or fallback to reformulation / error if unsupported)

Indicator constraints are the first implementation, serving as a proof of concept for this architecture.

### Relationship between constraint types

Some constraint types are special cases of others:

- **Indicator** is a special case of **Disjunction**: `binary_var = 1 → f(x) <= 0` is equivalent to `(binary_var = 0) ∨ (f(x) <= 0)`
- **Disjunction** is the most general form, where each child `C_i` can be any constraint (including another disjunction, indicator, etc.)
- **Bound Disjunction** (SCIP `cons_bounddisjunction`) is a lightweight special case of Disjunction where each literal is a single variable bound: `(x1 <= b1) ∨ (x2 >= b2) ∨ ...`

### Why first-class for all types, not just disjunction?

More specific constraint types are subsets of more general ones (e.g. indicator ⊂ disjunction), but solvers can exploit the structure of specific types for better performance:

- **Indicator vs Big-M reformulation**: Indicator avoids choosing a Big-M constant, gives tighter LP relaxation, and enables more effective branching
- **Indicator vs Disjunction**: Solvers have dedicated propagation and separation algorithms for indicator constraints that are more efficient than general disjunction handling
- **SOS1/OneHot**: Specialized branching strategies that outperform generic approaches

Therefore, OMMX needs to represent each constraint type as first-class, not reduce everything to the most general form. The role of OMMX is to:

1. **Preserve semantic structure**: represent what the user actually means, not a lossy reformulation
2. **Enable solver-specific conversion**: adapters translate each type to the most efficient solver-native representation
3. **Provide conversion routines** (future): automatically convert between constraint types when beneficial, preferring more specific (and faster) types when applicable. For example, detecting that a disjunction `(b=0) ∨ (f(x)<=0)` with binary `b` can be represented as an indicator constraint

The planned evolution path:
1. **Indicator** — binary condition → single constraint (this design)
2. **OneHot / SOS1** — migrate from `ConstraintHints` to first-class constraints
3. **Disjunction** — general OR of constraints, subsumes indicator as a special case
4. **Conversion routines** — detect and convert to more specific types for performance

## Design Decisions

### Indicator variable is binary only

The indicator variable must be a binary (0/1) decision variable. General conditions like `x == k` for integer `x` are not supported.

Rationale:
- Nearly all solvers (SCIP, Gurobi, CPLEX) restrict indicator variables to binary
- SCIP has `cons_superindicator` for general conditions, but even that requires a binary indicator variable with linking constraints
- General conditions can be modeled by introducing auxiliary binary variables

### Equality support

Both `f(x) <= 0` and `f(x) = 0` are supported as the conditional constraint.

When a solver only supports `<=` (e.g. SCIP's `addConsIndicator`), the adapter decomposes `f(x) = 0` into two indicator constraints:
```
binvar = 1 → f(x) <= 0
binvar = 1 → -f(x) <= 0
```

### Constraint ID space

Each constraint type has its own independent `ConstraintID` space. IDs only need to be unique within the same type:

```
(Standard,  10)  — regular constraint ID=10
(Indicator, 10)  — indicator constraint ID=10 (no conflict)
```

When referencing a constraint across types (e.g. in logs, APIs, or serialization), a `(ConstraintType, ConstraintID)` tuple is used. Uniqueness is enforced only between active and removed constraints of the **same type** (i.e. `constraints` ∩ `removed_constraints` = ∅, `indicator_constraints` ∩ `removed_indicator_constraints` = ∅).

### Instance API

`Instance` provides separate accessors for each constraint type:

- `instance.constraints` — regular constraints only (backward compatible)
- `instance.indicator_constraints` — indicator constraints only
- Future: `instance.sos1_constraints`, `instance.one_hot_constraints`, etc.

### Evaluation semantics

When evaluating an indicator constraint with a `State`:

- If `state[indicator_variable_id] == 1`: evaluate `f(x)` and check feasibility as usual
- If `state[indicator_variable_id] == 0`: always feasible, `evaluated_value` is still `f(x)` for diagnostics

The `EvaluatedConstraint` output uses the same structure. The `feasible` field reflects the conditional logic.

## SCIP Reference

### `cons_indicator` (PySCIPOpt `addConsIndicator`)

```python
model.addConsIndicator(
    cons,           # ExprCons: linear inequality (<= form only)
    binvar=binvar,  # Binary variable (or None to auto-create)
    activeone=True, # If False, constraint active when binvar=0
)
```

- Linear inequality only
- Binary indicator variable only
- `activeone=False` internally uses `SCIPgetNegatedVar`
- Each indicator constraint creates a slack variable internally

### `cons_superindicator`

```
x_i = 1 ⇒ C(x)
```

- Binary indicator variable, but `C(x)` can be any constraint type
- SCIP internally attempts to downgrade to `cons_indicator` when possible

### `cons_disjunction`

```
C1 ∨ C2 ∨ ... ∨ Cn
```

- Each `C_i` is an arbitrary SCIP constraint
- At least one `C_i` must be satisfied
- Indicator is a special case: `(binary_var = 0) ∨ (f(x) <= 0)`

PySCIPOpt API:
```python
c1 = model.addCons(x + y <= 10, addToModel=False)
c2 = model.addCons(x - y <= 5, addToModel=False)
model.addConsDisjunction([c1, c2])
```

### `cons_bounddisjunction`

```
(x1 ≤ b1) ∨ (x2 ≥ b2) ∨ ... ∨ (xn ≤ bn)
```

- Each literal is a single variable bound (not a general linear expression)
- Lightweight special case of disjunction
- Not exposed in PySCIPOpt

## Adapter Implementation (PySCIPOpt)

```python
def _set_constraints(self):
    # Regular constraints (existing code, unchanged)
    for constraint in self.instance.constraints:
        ...

    # Indicator constraints (new)
    for indicator in self.instance.indicator_constraints:
        binvar = self.varname_map[str(indicator.indicator_variable_id)]
        expr = self._make_linear_expr(indicator.function)
        if indicator.equality == Constraint.EQUAL_TO_ZERO:
            # Decompose into two <= indicators
            self.model.addConsIndicator(expr <= 0, binvar=binvar, ...)
            self.model.addConsIndicator(-expr <= 0, binvar=binvar, ...)
        else:
            self.model.addConsIndicator(expr <= 0, binvar=binvar, ...)
```

## Data Model

### Constraint lifecycle stages

Each constraint type goes through lifecycle stages: Created → Evaluated / Removed / Sampled.
Rather than defining separate types for each combination (e.g. `EvaluatedConstraint`, `EvaluatedIndicatorConstraint`, ...),
we parameterize constraint types by their stage using a trait:

```rust
trait ConstraintType {
    /// Data produced when evaluating a single state
    type EvaluatedData;
    /// Data produced when evaluating multiple samples
    type SampledData;
    // RemovedData is the same for all constraint types (reason + parameters)
}
```

Each constraint type defines what its evaluation result looks like:

```rust
// Regular constraint
struct Constraint<S: Stage> {
    id: ConstraintID,
    equality: Equality,
    function: Function,
    metadata: ConstraintMetadata,
    stage: S,
}

// Indicator constraint
struct IndicatorConstraint<S: Stage> {
    id: ConstraintID,
    indicator_variable: VariableID,  // must be binary
    equality: Equality,
    function: Function,
    metadata: ConstraintMetadata,
    stage: S,
}
```

Stages carry constraint-type-specific data:

```rust
// For regular Constraint:
//   Evaluated → { evaluated_value: f64, feasible: bool }
//   Sampled   → { evaluated_values: Sampled<f64>, feasible: BTreeMap<SampleID, bool> }

// For IndicatorConstraint:
//   Evaluated → { active: bool, evaluated_value: f64, feasible: bool }
//   Sampled   → { per-sample active/value/feasible }

// For all types:
//   Removed   → { removed_reason: String, removed_reason_parameters: ... }
```

Type aliases maintain readability:

```rust
type ActiveConstraint = Constraint<Created>;
type EvaluatedConstraint = Constraint<Evaluated>;
type RemovedConstraint = Constraint<Removed>;
```

The key insight is that `Removed` is uniform across all constraint types (just reason + parameters),
while `Evaluated` and `Sampled` are constraint-type-specific.
This can be modeled by having the `Stage` trait parameterized by the constraint type,
or by having each constraint type define its own associated evaluation data via `ConstraintType`.

The exact trait design needs further prototyping.

### Instance

```rust
pub struct Instance {
    // existing fields...
    constraints: BTreeMap<ConstraintID, Constraint<Created>>,
    removed_constraints: BTreeMap<ConstraintID, Constraint<Removed>>,

    // new
    indicator_constraints: BTreeMap<ConstraintID, IndicatorConstraint<Created>>,
    removed_indicator_constraints: BTreeMap<ConstraintID, IndicatorConstraint<Removed>>,

    // future
    // sos1_constraints: BTreeMap<ConstraintID, Sos1Constraint<Created>>,
    // one_hot_constraints: BTreeMap<ConstraintID, OneHotConstraint<Created>>,
}
```

### Solution / SampleSet

```rust
pub struct Solution {
    // evaluated_constraints holds results for ALL constraint types
    evaluated_constraints: BTreeMap<ConstraintID, EvaluatedConstraint>,
    evaluated_indicator_constraints: BTreeMap<ConstraintID, EvaluatedIndicatorConstraint>,
    // ...
}
```

### Protobuf (deferred)

Protobuf schema changes are deferred. The initial implementation focuses on the Rust/Python API and adapter integration. Serialization format will be designed after the API stabilizes.

## Resolved Design Points

### `partial_evaluate`

When the indicator variable is fixed:
- **Fixed to 1**: convert to a regular constraint (the condition is always active)
- **Fixed to 0**: remove (always satisfied, no effect)
- **Not fixed**: partial evaluate only the function part, keep the indicator structure

### `required_ids`

The indicator variable ID is included in `required_ids`. This method returns all variable IDs needed to evaluate the constraint, and the indicator variable is necessary for evaluation.

### Adapter fallback

Handled by the OMMX core capability model (see [Adapter capability model](#adapter-capability-model)):
- If adapter declares indicator support → pass through
- If not → OMMX core auto-converts (e.g. Big-M), or errors if no conversion exists

### Evaluation result

The current `EvaluatedConstraint` has a single `evaluated_value: f64` and `feasible: bool`. For indicator constraints, the evaluation result needs to be a structured value:

- **Indicator OFF** (indicator variable = 0): always feasible, no meaningful scalar value
- **Indicator ON** (indicator variable = 1): `evaluated_value` is `f(x)`, feasibility determined by equality

This means `EvaluatedConstraint` needs to accommodate structured results, not just a flat `f64`. The exact representation (enum, optional fields, etc.) requires further design, especially considering that disjunction constraints will have even more complex evaluation results (a vector of child evaluations).

### `removed_constraints`

Removing an indicator constraint (constraint relaxation) is straightforward in concept: move it out of `indicator_constraints`. However, the current `RemovedConstraint` struct wraps a regular `Constraint`:

```rust
pub struct RemovedConstraint {
    pub constraint: Constraint,  // only regular Constraint
    pub removed_reason: String,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}
```

This needs to be generalized to wrap any constraint type. Possible approaches:
- A `RemovedConstraint` enum that can hold any constraint type
- A generic `Removed<T>` wrapper
- Separate collections: `removed_constraints`, `removed_indicator_constraints`, etc.

This mirrors the same multi-type pattern as the active constraints in `Instance`.

## Python API

### Constructing indicator constraints

All three approaches are supported:

```python
# Direct construction
ic = IndicatorConstraint(
    indicator_variable=b,
    function=x + y,
    equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
)

# From existing constraint expression
ic = (x + y <= 0).with_indicator(b)

# At Instance construction
instance = Instance.from_components(
    ...,
    indicator_constraints=[ic1, ic2, ...],
)
```

### ConstraintHints coexistence

The existing `ConstraintHints` (OneHot, SOS1) continues to work alongside first-class indicator constraints. When SOS1/OneHot are later promoted to first-class constraint types, `ConstraintHints` will be deprecated. During the transition period, both paths coexist.

### Adapter capability model

Adapters declare which constraint types they support (their "capability"). The OMMX core layer is responsible for bridging the gap between the Instance and the adapter's capability:

```
Instance (may contain any constraint types)
    │
    ▼
OMMX core: check adapter capability
    │  - supported type → pass through
    │  - unsupported type → auto-convert (e.g. Big-M reformulation)
    │  - unconvertible → error
    ▼
Adapter: receives only constraint types it declared support for
```

Example capability declarations:

```python
class OMMXPySCIPOptAdapter(SolverAdapter):
    # Supports regular + indicator + SOS1
    supported_constraint_types = {Standard, Indicator, Sos1}

class OMMXHighsAdapter(SolverAdapter):
    # Supports regular only
    supported_constraint_types = {Standard}
```

Benefits:
- **Adapter authors** only declare capability, no need to handle unknown types
- **New constraint types** in OMMX do not require changes to existing adapters
- **Auto-conversion** (e.g. indicator → Big-M) is implemented once in OMMX core, not per adapter
- **Adapters are guaranteed** to only receive constraint types they support

### `feasible` / `feasible_relaxed`

Removed indicator constraints follow the same semantics as removed regular constraints:
- `feasible`: considers ALL constraints including removed (indicator and regular)
- `feasible_relaxed`: only considers active constraints

## Open Questions

- How does disjunction `evaluate` work? All children must be evaluated, feasible if at least one child is feasible. The evaluation result would be a collection of child results.
- Should there be a unified iteration API across all constraint types (e.g. `instance.all_constraint_ids()`) in addition to the per-type accessors?
