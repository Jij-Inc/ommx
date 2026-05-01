# Rust SDK Migration Guide

This document covers migration of the OMMX Rust SDK (`ommx` crate) across major versions.

- [v3 (Stage Pattern)](#rust-sdk-v3-stage-pattern-migration-guide) — Constraint lifecycle stage parameterization

---

# Rust SDK v3 Stage Pattern Migration Guide

This section covers the migration to stage-parameterized constraints
landed in `3.0.0-alpha.1`.

## Overview

`Constraint` is now generic over a lifecycle stage, and its
`ConstraintID` lives on the enclosing collection key rather than on the
struct itself:

```rust,ignore
pub struct Constraint<S: Stage<Self> = Created> {
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

Removed constraints are managed at the collection level —
`ConstraintCollection` stores them as `(Constraint<Created>, RemovedReason)`
pairs. "Removed" is not itself a stage.

## Breaking Changes

### 1. Constraint Field Access

Fields that were previously on the struct directly are now split
between common fields and stage-specific data. The `id` field is gone
entirely — look it up via the enclosing `BTreeMap` key.

**Common fields**:
```rust,ignore
// ❌ Before
constraint.id        // ConstraintID

// ✅ After — IDs live on collection keys
for (id, constraint) in instance.constraints() {
    // `id: &ConstraintID`, `constraint: &Constraint`
}

// ✅ Unchanged
constraint.equality  // Equality
```

**Metadata fields** (moved to `metadata`):
```rust,ignore
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
```rust,ignore
// ❌ Before
constraint.function

// ✅ After (method)
constraint.function()       // &Function
constraint.function_mut()   // &mut Function

// ✅ After (direct field)
constraint.stage.function   // Function
```

**Evaluated stage** — evaluation result access:
```rust,ignore
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
```rust,ignore
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
```rust,ignore
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

// ✅ After — no `id` field
Constraint {
    equality: Equality::EqualToZero,
    metadata: ConstraintMetadata::default(),
    stage: CreatedData { function },
}

// ✅ Factory methods no longer take an ID
Constraint::equal_to_zero(function)
Constraint::less_than_or_equal_to_zero(function)

// ✅ The ID attaches when you insert into a BTreeMap
let mut constraints = BTreeMap::new();
constraints.insert(ConstraintID::from(1), Constraint::equal_to_zero(function));
```

**Removed constraints** are no longer constructed as `Constraint<Removed>`. They are stored as `(Constraint<Created>, RemovedReason)` tuples in `ConstraintCollection`:
```rust,ignore
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
```rust,ignore
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

// ✅ After — no `id` field; insert with the key when storing
Constraint {
    equality, metadata,
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

```rust,ignore
// ❌ Before (v2)
removed.constraint.id
removed.constraint.equality
removed.constraint.function
removed.removed_reason              // String
removed.removed_reason_parameters   // FnvHashMap<String, String>

// ✅ After — access via the tuple; the ID comes from the map key
let (constraint, reason) = collection.removed().get(&id).unwrap();
// id is the BTreeMap key you looked it up by
constraint.equality
constraint.function()
reason.reason
reason.parameters
```

### 4. RemovedReason Struct

`removed_reason: String` + `removed_reason_parameters: FnvHashMap<String, String>` are consolidated into a single struct:

```rust,ignore
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
```rust,ignore
// These still work
instance.constraints()           // &BTreeMap<ConstraintID, Constraint>
instance.removed_constraints()   // &BTreeMap<ConstraintID, (Constraint, RemovedReason)>

// New: access the full collection
instance.constraint_collection() // &ConstraintCollection<Constraint>
```

For mutable access, downstream code goes through invariant-safe
`Instance` / `ParametricInstance` methods (`add_constraint`,
`insert_constraint`, `relax_constraint`, `restore_constraint`, …) —
these validate that every `id` referenced by the constraint exists in
`decision_variables` and keep the active / removed maps disjoint. The
raw `active_mut()` / `removed_mut()` mutators on
`ConstraintCollection<T>` are `pub(crate)` and not callable from
outside the crate.

### 6. getset Removal

`EvaluatedConstraint` and `SampledConstraint` no longer use the `getset` crate. All fields are accessed directly via `self.id`, `self.equality`, `self.metadata`, and `self.stage.*`.

Methods like `.id()`, `.equality()`, `.evaluated_value()`, `.feasible()` are **removed**. Use field access instead.

### 7. Error Surface Call-Site Rewrites

See the [release note](crate::doc::release_note::v3_0_0_alpha_1) for the
rationale and the [error handling tutorial](crate::doc::tutorial::error_handling)
for the `ommx::Result` / signal-type / fail-site-macro story. This
section only lists the mechanical call-site rewrites you need to apply
when upgrading a crate that was on v2.

**Deleted error enums.** The types below no longer exist. Match arms
that inspected their variants should switch to string inspection (if
you really cared) or just propagate via `?`:

- `ommx::InstanceError`
- `ommx::MpsParseError`, `ommx::MpsWriteError`
- `ommx::StateValidationError`, `ommx::LogEncodingError`
- `ommx::UnknownSampleIDError` — replaced by `Option<T>` on key-lookup methods
- `ommx::ParseErrorReason` — the variant enum inside the old `ommx::QplibParseError`

```rust,ignore
// ❌ Before (v2)
match decode(bytes) {
    Err(InstanceError::DuplicateConstraintID(id)) => { ... }
    Err(InstanceError::UndefinedVariable(v)) => { ... }
    Err(e) => return Err(e),
    Ok(x) => x,
}

// ✅ After (v3) — either propagate, or inspect the rendered message
let instance = decode(bytes)?;
```

**Moved / renamed error types:**

- `ommx::QplibParseError` → `ommx::qplib::QplibParseError`. The type is
  slimmer (1-based `line_num` + rendered `message`, no variant enum),
  and no longer re-exported at the crate root.

**Key lookups now return `Option<T>`:**

```rust,ignore
// ❌ Before — typed Err when the key was missing
let solution: Solution = sample_set
    .get(id)
    .map_err(|UnknownSampleIDError { .. }| /* handle */)?;

// ✅ After — Option, lifted at the boundary if your caller wants Result
let solution: Solution = sample_set
    .get(id)
    .ok_or_else(|| ommx::error!("unknown sample id {id:?}"))?;
```

**Signal-type recovery is unchanged in syntax** — it just now flows
through `ommx::Error` instead of a bespoke enum:

```rust,ignore
match instance.propagate(&state, atol) {
    Err(e) if e.is::<ommx::InfeasibleDetected>() => { /* handle */ }
    Err(e) => return Err(e),
    Ok(outcome) => { /* ... */ }
}
```

## New Types

### ConstraintType Trait

A type family mapping lifecycle stages to concrete types (HKT defunctionalization):

```rust,ignore
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

```rust,ignore
pub trait EvaluatedConstraintBehavior {
    type ID;
    fn is_feasible(&self) -> bool;
}

pub trait SampledConstraintBehavior {
    type ID;
    type Evaluated;
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool>;
    fn get(&self, sample_id: SampleID) -> Option<Self::Evaluated>;
}
```

Neither trait exposes the constraint's ID — it lives on the enclosing
`BTreeMap` key. `is_removed()` is similarly absent from the traits;
use `EvaluatedCollection::is_removed(&id)` or
`SampledCollection::is_removed(&id)` instead.

### ConstraintCollection

Generic collection of active + removed constraints. Also implements `Evaluate`:

```rust,ignore
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
}

// Methods (public)
collection.active()                    // &BTreeMap<T::ID, T::Created>
collection.removed()                   // &BTreeMap<T::ID, (T::Created, RemovedReason)>
collection.metadata()                  // &ConstraintMetadataStore<T::ID>
collection.into_parts()                // (active, removed, metadata)
// Mutation goes through Instance / ParametricInstance methods so
// invariants (active/removed disjointness, variable-id validity)
// are enforced; the raw `active_mut` / `removed_mut` / `insert_with`
// primitives on this type are `pub(crate)`.

// Evaluate trait impl
collection.evaluate(state, atol)           // EvaluatedCollection<T>
collection.evaluate_samples(samples, atol) // SampledCollection<T>
collection.partial_evaluate(state, atol)   // only active constraints
collection.required_ids()                  // VariableIDSet
```

Removed constraints are just `Created` constraints paired with a `RemovedReason`. The `Removed` stage type no longer exists.

### EvaluatedCollection / SampledCollection

Generic wrappers for evaluation results, used in `Solution` and `SampleSet`:

```rust,ignore
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

```rust,ignore
pub struct ConstraintMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
    /// Chain of transformations that produced this constraint.
    /// Empty for directly-authored constraints; populated when e.g. an
    /// IndicatorConstraint is promoted to a regular Constraint.
    pub provenance: Vec<Provenance>,
}
```

## Migration Checklist

- [ ] Remove `constraint.id` reads — look up the ID via the enclosing `BTreeMap<ConstraintID, _>` key instead
- [ ] Update `Constraint::equal_to_zero(id, function)` / `Constraint::less_than_or_equal_to_zero(id, function)` → drop the ID argument (`Constraint::equal_to_zero(function)`), insert with the key
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
