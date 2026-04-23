# Rust SDK Migration Guide

This document covers migration of the OMMX Rust SDK (`ommx` crate) across major versions.

- [v3 (Stage Pattern)](#rust-sdk-v3-stage-pattern-migration-guide) — Constraint lifecycle stage parameterization

---

# Rust SDK v3 Stage Pattern Migration Guide

This section covers the migration to stage-parameterized constraints, introduced in the `refactor/constraint-stage-pattern` branch.

## Overview

`Constraint` is now generic over a lifecycle stage:

```rust
pub struct Constraint<S: Stage<Self> = Created> {
    pub id: ConstraintID,
    pub equality: Equality,
    pub metadata: ConstraintMetadata,
    pub stage: S::Data,
}
```

Three lifecycle stages are defined:

| Type alias | Full type | Stage data |
|---|---|---|
| `Constraint` | `Constraint<Created>` | `CreatedData { function }` |
| `EvaluatedConstraint` | `Constraint<Evaluated>` | `EvaluatedData { evaluated_value, feasible, ... }` |
| `SampledConstraint` | `Constraint<stage::Sampled>` | `SampledData { evaluated_values, feasible, ... }` |

Removed constraints are managed at the collection level — `ConstraintCollection` stores them as `(Constraint<Created>, RemovedReason)` pairs.

## Breaking Changes

### 1. Constraint Field Access

Fields that were previously on the struct directly are now split between common fields and stage-specific data.

**Common fields** (unchanged access):
```rust
constraint.id        // ConstraintID
constraint.equality  // Equality
```

**Metadata fields** (moved to `metadata`):
```rust
// ❌ Before
constraint.name
constraint.subscripts
constraint.parameters
constraint.description

// ✅ After
constraint.metadata.name
constraint.metadata.subscripts
constraint.metadata.parameters
constraint.metadata.description
```

**Created stage** — function access:
```rust
// ❌ Before
constraint.function

// ✅ After (method)
constraint.function()       // &Function
constraint.function_mut()   // &mut Function

// ✅ After (direct field)
constraint.stage.function   // Function
```

**Evaluated stage** — evaluation result access:
```rust
// ❌ Before (getset methods)
*evaluated.evaluated_value()
*evaluated.feasible()
evaluated.dual_variable
evaluated.removed_reason()
evaluated.used_decision_variable_ids()

// ✅ After (direct stage field access)
evaluated.stage.evaluated_value
evaluated.stage.feasible
evaluated.stage.dual_variable
evaluated.stage.used_decision_variable_ids
```

`removed_reason` is no longer on evaluated/sampled constraints — it's managed by `EvaluatedCollection` / `SampledCollection` via `collection.removed_reasons()` and `collection.is_removed(&id)`.

**Sampled stage** — same pattern:
```rust
// ❌ Before
*sampled.evaluated_values()
sampled.feasible()
sampled.dual_variables

// ✅ After
sampled.stage.evaluated_values
sampled.stage.feasible
sampled.stage.dual_variables
```

### 2. Struct Literal Construction

**Constraint (Created)**:
```rust
// ❌ Before
Constraint {
    id: ConstraintID::from(1),
    function,
    equality: Equality::EqualToZero,
    name: None,
    subscripts: Vec::new(),
    parameters: FnvHashMap::default(),
    description: None,
}

// ✅ After
Constraint {
    id: ConstraintID::from(1),
    equality: Equality::EqualToZero,
    metadata: ConstraintMetadata::default(),
    stage: CreatedData { function },
}

// ✅ Or use factory methods (unchanged)
Constraint::equal_to_zero(ConstraintID::from(1), function)
Constraint::less_than_or_equal_to_zero(ConstraintID::from(1), function)
```

**Removed constraints** are no longer constructed as `Constraint<Removed>`. They are stored as `(Constraint<Created>, RemovedReason)` tuples in `ConstraintCollection`:
```rust
// ❌ Before (v2)
let removed = RemovedConstraint {
    constraint: inner_constraint,
    removed_reason: "reason".to_string(),
    removed_reason_parameters: Default::default(),
};

// ✅ After — use Instance::relax_constraint() or store tuples directly
instance.relax_constraint(id, "reason".to_string(), [])?;

// Or if constructing directly:
let removed: (Constraint, RemovedReason) = (
    constraint,
    RemovedReason {
        reason: "reason".to_string(),
        parameters: Default::default(),
    },
);
```

**EvaluatedConstraint**:
```rust
// ❌ Before
EvaluatedConstraint {
    id, equality, metadata,
    evaluated_value,
    feasible,
    dual_variable: None,
    used_decision_variable_ids,
    removed_reason: None,
    removed_reason_parameters: FnvHashMap::default(),
}

// ✅ After
Constraint {
    id, equality, metadata,
    stage: EvaluatedData {
        evaluated_value,
        feasible,
        dual_variable: None,
        used_decision_variable_ids,
    },
}
```

### 3. RemovedConstraint Removed

`RemovedConstraint` type alias no longer exists. Removed constraints are stored as `(Constraint<Created>, RemovedReason)` tuples in `ConstraintCollection`.

```rust
// ❌ Before (v2)
removed.constraint.id
removed.constraint.equality
removed.constraint.function
removed.removed_reason              // String
removed.removed_reason_parameters   // FnvHashMap<String, String>

// ✅ After — access via the tuple
let (constraint, reason) = collection.removed().get(&id).unwrap();
constraint.id
constraint.equality
constraint.function()
reason.reason
reason.parameters
```

### 4. RemovedReason Struct

`removed_reason: String` + `removed_reason_parameters: FnvHashMap<String, String>` are consolidated into a single struct:

```rust
pub struct RemovedReason {
    pub reason: String,
    pub parameters: FnvHashMap<String, String>,
}
```

`RemovedReason` is stored at the collection level, not on individual constraints:
- `ConstraintCollection.removed()` → `&BTreeMap<ID, (T::Created, RemovedReason)>`
- `EvaluatedCollection.removed_reasons()` → `&BTreeMap<ID, RemovedReason>`
- `SampledCollection.removed_reasons()` → `&BTreeMap<ID, RemovedReason>`

### 5. Instance Fields

`Instance.constraints` and `Instance.removed_constraints` fields are replaced by `constraint_collection: ConstraintCollection<Constraint>`.

Accessor methods are preserved for backward compatibility:
```rust
// These still work
instance.constraints()           // &BTreeMap<ConstraintID, Constraint>
instance.removed_constraints()   // &BTreeMap<ConstraintID, (Constraint, RemovedReason)>

// New: access the full collection
instance.constraint_collection() // &ConstraintCollection<Constraint>
```

For internal/mutable access:
```rust
// ❌ Before
self.constraints.values_mut()
self.removed_constraints.entry(id)

// ✅ After
self.constraint_collection.active_mut().values_mut()
self.constraint_collection.removed_mut().entry(id)
```

### 6. getset Removal

`EvaluatedConstraint` and `SampledConstraint` no longer use the `getset` crate. All fields are accessed directly via `self.id`, `self.equality`, `self.metadata`, and `self.stage.*`.

Methods like `.id()`, `.equality()`, `.evaluated_value()`, `.feasible()` are **removed**. Use field access instead.

### 7. Unified Error Surface (`ommx::Result` + `ommx::Error`)

The crate now returns a single error type across its public API:

```rust
// ❌ Before (v2): a mix of anyhow::Result, thiserror enums, and one-off error types
fn some_public_api() -> Result<Instance, InstanceError> { ... }

// ✅ After (v3): no domain-specific enum on the public surface
fn some_public_api() -> ommx::Result<Instance> { ... }
```

`ommx::Error` and `ommx::Result` are re-exports of `anyhow::Error` and `anyhow::Result`, so `anyhow::Result<T>` and `ommx::Result<T>` are the same type — the v3 change deleted the domain-specific enums, not the anyhow alias. Prefer `ommx::Result<T>` in new code so the crate name is visible on the API surface, but there is no reason to rewrite existing `anyhow::Result<T>` signatures. Equivalently:

```rust
// These still work
err.chain()
err.root_cause()
err.downcast_ref::<MySignal>()
err.is::<MySignal>()

// Crate boundary: propagate with `?` as usual
fn my_fn() -> ommx::Result<()> {
    let inst = some_public_api()?;  // anyhow-based chain
    Ok(())
}
```

#### Deleted enums

The following typed error enums have been removed. Callers that matched on discriminants should switch to `err.to_string()` inspection or (for signal types) `err.downcast_ref::<T>()`:

- `ommx::InstanceError` (~20 variants covering `Instance` / `ParametricInstance` invariants)
- `ommx::MpsParseError`, `ommx::MpsWriteError`
- `ommx::ParseErrorReason` (the variant enum inside the old `ommx::QplibParseError` — the struct itself has been replaced, see below)
- `ommx::StateValidationError`, `ommx::LogEncodingError`
- `ommx::UnknownSampleIDError` (now expressed as `Option<T>` on key-lookup methods)
- The `ommx::Error` newtype from an earlier v3 alpha; it is now an alias for `anyhow::Error`.

#### Narrow-domain structured errors kept

Two structured error types stay `pub` because they carry *positional* metadata that downstream code can consume programmatically:

- **`ommx::ParseError`** — breadcrumb-bearing proto-tree parse error. The `Parse` trait signature still returns `Result<_, ParseError>`; see the "`Parse` trait and `ParseError`" note in the PR description for the kept-intentionally rationale.
- **`ommx::qplib::QplibParseError`** — a slimmer replacement for the old `ommx::QplibParseError` + `ommx::ParseErrorReason` pair. Carries a 1-based `line_num` plus a rendered `message`. Callers that used to match on `ParseErrorReason` variants should now inspect `message`, or use `err.downcast_ref::<ommx::qplib::QplibParseError>()` to surface `line_num` for editor-style diagnostics. Note the new type lives under the `ommx::qplib` module (no longer re-exported at the crate root).

#### Signal types (kept)

A small set of structured errors remain `pub` because they encode recoverable conditions callers may want to detect:

- `ommx::InfeasibleDetected`
- `ommx::DuplicatedSampleIDError`
- `ommx::CoefficientError`, `ommx::BoundError`, `ommx::AtolError`
- `ommx::DecisionVariableError`, `ommx::SubstitutionError`, `ommx::SolutionError`, `ommx::SampleSetError`

Recover them by downcast:

```rust
match instance.propagate(&state, atol) {
    Err(e) if e.is::<ommx::InfeasibleDetected>() => { /* handle */ }
    Err(e) => return Err(e),
    Ok(outcome) => { /* ... */ }
}
```

#### Parse trait and `ParseError` (kept)

`ParseError` is intentionally not collapsed into `ommx::Error`. It carries structured `Vec<ParseContext>` breadcrumbs that walk the proto tree field-by-field, which is useful metadata rather than a discriminant downstream code ignores. `ParseError` implements `std::error::Error`, so it flows into `ommx::Result<T>` via `?` at the crate boundary:

```rust
fn load_something(bytes: &[u8]) -> ommx::Result<Instance> {
    let v1_inst: v1::Instance = Message::decode(bytes)?;
    let inst: Instance = v1_inst.parse(&())?;  // ParseError → anyhow::Error
    Ok(inst)
}
```

#### Diagnostic-emitting macros

The crate exposes `ommx::bail!` / `ommx::error!` / `ommx::ensure!` macros that bundle two actions every failure site needs:

1. Emit a `tracing::error!` event (visible to any subscriber).
2. Produce an `anyhow::Error` with the rendered message.

```rust
// Plain message — tracing event + anyhow::Error share the format string
ommx::bail!("invalid OBJSENSE: {s}");

// Structured tracing fields via `{ field = value, … }`
ommx::bail!(
    { section, size },
    "invalid field size ({size}) in MPS section '{section}'",
);

// Signal-style expression — no tracing event, since the caller typically
// recovers it by downcast
ommx::bail!(InfeasibleDetected);
```

These are mainly for internal fail sites, but downstream crates may use them too.

## New Types

### ConstraintType Trait

A type family mapping lifecycle stages to concrete types (HKT defunctionalization):

```rust
pub trait ConstraintType {
    type ID: Clone + Copy + Ord + Hash + Debug;
    type Created: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>
        + Clone + Debug + PartialEq;
    type Evaluated: EvaluatedConstraintBehavior<ID = Self::ID>;
    type Sampled: SampledConstraintBehavior<ID = Self::ID, Evaluated = Self::Evaluated>;
}

// Regular constraints
impl ConstraintType for Constraint {
    type ID = ConstraintID;
    // ...
}

// Indicator constraints
impl ConstraintType for IndicatorConstraint {
    type ID = IndicatorConstraintID;
    // ...
}
```

### Behavior Traits

Two traits define common behavior for evaluated and sampled constraints:

```rust
pub trait EvaluatedConstraintBehavior {
    type ID;
    fn constraint_id(&self) -> Self::ID;
    fn is_feasible(&self) -> bool;
}

pub trait SampledConstraintBehavior {
    type ID;
    type Evaluated;
    fn constraint_id(&self) -> Self::ID;
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool>;
    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated>;
}
```

`is_removed()` has been removed from these traits — use `EvaluatedCollection::is_removed(&id)` or `SampledCollection::is_removed(&id)` instead.

### ConstraintCollection

Generic collection of active + removed constraints. Also implements `Evaluate`:

```rust
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
}

// Methods
collection.active()                    // &BTreeMap<T::ID, T::Created>
collection.removed()                   // &BTreeMap<T::ID, (T::Created, RemovedReason)>
collection.active_mut()                // &mut BTreeMap
collection.removed_mut()               // &mut BTreeMap
collection.into_parts()                // (active, removed)

// Evaluate trait impl
collection.evaluate(state, atol)           // EvaluatedCollection<T>
collection.evaluate_samples(samples, atol) // SampledCollection<T>
collection.partial_evaluate(state, atol)   // only active constraints
collection.required_ids()                  // VariableIDSet
```

Removed constraints are just `Created` constraints paired with a `RemovedReason`. The `Removed` stage type no longer exists.

### EvaluatedCollection / SampledCollection

Generic wrappers for evaluation results, used in `Solution` and `SampleSet`:

```rust
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
}

pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
}

// Both Deref to BTreeMap<T::ID, T::Evaluated/Sampled> for backward-compatible access
// and provide feasibility and removal methods:
collection.is_feasible()               // all constraints feasible
collection.is_feasible_relaxed()       // all non-removed constraints feasible
collection.is_removed(&id)             // check if a constraint was removed
collection.removed_reasons()           // &BTreeMap<T::ID, RemovedReason>
collection.into_parts()                // (constraints, removed_reasons)
```

### ConstraintMetadata

Common metadata extracted from the constraint:

```rust
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}
```

## Migration Checklist

- [ ] Update `constraint.function` → `constraint.function()` or `constraint.stage.function`
- [ ] Update `constraint.name` → `constraint.metadata.name` (and `subscripts`, `parameters`, `description`)
- [ ] Update `evaluated.evaluated_value()` → `evaluated.stage.evaluated_value` (and other getset methods)
- [ ] Update `RemovedConstraint` construction → `(Constraint, RemovedReason)` tuple
- [ ] Update `removed.constraint.xxx` → `removed.0.xxx` (tuple access)
- [ ] Update `removed_reason` / `removed_reason_parameters` → `RemovedReason { reason, parameters }`
- [ ] Update `evaluated.removed_reason()` → `collection.removed_reasons().get(&id)`
- [ ] Update struct literals to use `stage: CreatedData { ... }` / `EvaluatedData { ... }` / etc.
- [ ] Update `self.constraints` / `self.removed_constraints` → `self.constraint_collection.active()` / `.removed()`
- [ ] Remove any `getset` usage for constraint types
- [ ] Update any `InstanceError` / `MpsParseError` / `QplibParseError` / `StateValidationError` / `LogEncodingError` / `UnknownSampleIDError` matches → inspect `err.to_string()` or use `err.downcast_ref::<T>()` for signal types
- [ ] Replace `Result<T, UnknownSampleIDError>` key-lookup methods with `Option<T>` on the call site
