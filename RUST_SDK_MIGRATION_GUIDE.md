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
    fn get(&self, sample_id: SampleID) -> Result<Self::Evaluated, UnknownSampleIDError>;
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
