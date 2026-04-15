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

All four lifecycle stages are unified:

| Type alias | Full type | Stage data |
|---|---|---|
| `Constraint` | `Constraint<Created>` | `CreatedData { function }` |
| `RemovedConstraint` | `Constraint<Removed>` | `RemovedData { function, removed_reason }` |
| `EvaluatedConstraint` | `Constraint<Evaluated>` | `EvaluatedData { evaluated_value, feasible, ... }` |
| `SampledConstraint` | `Constraint<stage::Sampled>` | `SampledData { evaluated_values, feasible, ... }` |

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

**Created/Removed stage** — function access:
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
evaluated.stage.removed_reason
evaluated.stage.used_decision_variable_ids
```

**Sampled stage** — same pattern:
```rust
// ❌ Before
*sampled.evaluated_values()
sampled.feasible()
sampled.dual_variables
sampled.removed_reason()

// ✅ After
sampled.stage.evaluated_values
sampled.stage.feasible
sampled.stage.dual_variables
sampled.stage.removed_reason
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

**RemovedConstraint**:
```rust
// ❌ Before
RemovedConstraint {
    constraint: inner_constraint,
    removed_reason: "reason".to_string(),
    removed_reason_parameters: Default::default(),
}

// ✅ After
Constraint {
    id: inner_constraint.id,
    equality: inner_constraint.equality,
    metadata: inner_constraint.metadata,
    stage: RemovedData {
        function: inner_constraint.stage.function,
        removed_reason: RemovedReason {
            reason: "reason".to_string(),
            parameters: Default::default(),
        },
    },
}
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
        removed_reason: None,
    },
}
```

### 3. RemovedConstraint Field Access

`RemovedConstraint` no longer wraps a `Constraint`. All fields are at the top level.

```rust
// ❌ Before
removed.constraint.id
removed.constraint.equality
removed.constraint.function
removed.removed_reason
removed.removed_reason_parameters

// ✅ After
removed.id
removed.equality
removed.stage.function     // or removed.function()
removed.stage.removed_reason.reason
removed.stage.removed_reason.parameters
```

### 4. RemovedReason Struct

`removed_reason: String` + `removed_reason_parameters: FnvHashMap<String, String>` are consolidated into a single struct:

```rust
pub struct RemovedReason {
    pub reason: String,
    pub parameters: FnvHashMap<String, String>,
}
```

In `RemovedData`: `pub removed_reason: RemovedReason`
In `EvaluatedData`/`SampledData`: `pub removed_reason: Option<RemovedReason>`

### 5. Instance Fields

`Instance.constraints` and `Instance.removed_constraints` fields are replaced by `constraint_collection: ConstraintCollection<Constraint>`.

Accessor methods are preserved for backward compatibility:
```rust
// These still work
instance.constraints()           // &BTreeMap<ConstraintID, Constraint>
instance.removed_constraints()   // &BTreeMap<ConstraintID, RemovedConstraint>

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
    type Removed: Evaluate<Output = Self::Evaluated, SampledOutput = Self::Sampled>
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

`HasConstraintID` is replaced by two purpose-specific traits:

```rust
pub trait EvaluatedConstraintBehavior {
    type ID;
    fn constraint_id(&self) -> Self::ID;
    fn is_feasible(&self) -> bool;
    fn is_removed(&self) -> bool;
}

pub trait SampledConstraintBehavior {
    type ID;
    type Evaluated;
    fn constraint_id(&self) -> Self::ID;
    fn is_feasible_for(&self, sample_id: SampleID) -> Option<bool>;
    fn is_removed(&self) -> bool;
    fn get(&self, sample_id: SampleID) -> Result<Self::Evaluated, UnknownSampleIDError>;
}
```

### ConstraintCollection

Generic collection of active + removed constraints. Also implements `Evaluate`:

```rust
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, T::Removed>,
}

// Methods
collection.active()                    // &BTreeMap<T::ID, T::Created>
collection.removed()                   // &BTreeMap<T::ID, T::Removed>
collection.active_mut()                // &mut BTreeMap
collection.removed_mut()               // &mut BTreeMap
collection.into_parts()                // (active, removed)

// Evaluate trait impl
collection.evaluate(state, atol)           // EvaluatedCollection<T>
collection.evaluate_samples(samples, atol) // SampledCollection<T>
collection.partial_evaluate(state, atol)   // only active constraints
collection.required_ids()                  // VariableIDSet
```

### EvaluatedCollection / SampledCollection

Generic wrappers for evaluation results, used in `Solution` and `SampleSet`:

```rust
// Solution uses:
evaluated_constraints: EvaluatedCollection<Constraint>,
evaluated_indicator_constraints: EvaluatedCollection<IndicatorConstraint>,

// SampleSet uses:
constraints: SampledCollection<Constraint>,
indicator_constraints: SampledCollection<IndicatorConstraint>,

// Both Deref to BTreeMap<T::ID, T::Evaluated/Sampled> for backward-compatible access
// and provide feasibility methods:
collection.is_feasible()             // EvaluatedCollection
collection.is_feasible_relaxed()     // EvaluatedCollection
collection.is_feasible_for(id)       // SampledCollection
collection.is_feasible_relaxed_for(id) // SampledCollection
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
- [ ] Update `removed.constraint.xxx` → `removed.xxx` or `removed.stage.xxx`
- [ ] Update `removed_reason` / `removed_reason_parameters` → `RemovedReason { reason, parameters }`
- [ ] Update struct literals to use `stage: CreatedData { ... }` / `EvaluatedData { ... }` / etc.
- [ ] Update `self.constraints` / `self.removed_constraints` → `self.constraint_collection.active()` / `.removed()`
- [ ] Remove any `getset` usage for constraint types
