# Rust SDK Migration Guide

This document covers migration of the OMMX Rust SDK (`ommx` crate) across major versions.

- [v3 (Stage Pattern)](#rust-sdk-v3-stage-pattern-migration-guide) — Constraint lifecycle stage parameterization
- [v3 (Artifact API)](#rust-sdk-v3-artifact-api-migration-guide) — Local registry / archive draft split and renames

---

# Rust SDK v3 Stage Pattern Migration Guide

This section covers the migration to stage-parameterized constraints
landed in `3.0.0-alpha.1`.

## Overview

`Constraint` is now generic over a lifecycle stage, its `ConstraintID`
lives on the enclosing collection key rather than on the struct itself,
and constraint context (`ModelingLabel` plus constraint `provenance`)
lives in a Struct-of-Arrays store on the enclosing
collection — not on the per-constraint struct:

```rust,ignore
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub stage: S::Data,
}
```

Three lifecycle stages are defined:

| Type alias | Full type | Stage data |
|---|---|---|
| `Constraint` | `Constraint<Created>` | `CreatedData { function }` |
| `EvaluatedConstraint` | `Constraint<Evaluated>` | `EvaluatedData { evaluated_value, feasible, ... }` |
| `SampledConstraint` | `Constraint<SampledStage>` | `SampledData { evaluated_values, feasible, ... }` |

Removed constraints are managed at the collection level —
`ConstraintCollection` stores them as `(Constraint<Created>, RemovedReason)`
pairs. "Removed" is not itself a stage.

`DecisionVariable`, `IndicatorConstraint<S>`, `OneHotConstraint<S>`,
`Sos1Constraint<S>`, and `NamedFunction` got the same SoA treatment —
each lost its inline label/context fields, and the per-host label/context store
is queried through narrow per-collection accessors on `Instance` /
`ParametricInstance` (see [Modeling labels and constraint context](#modeling-labels-and-constraint-context)).
Decision variables and named functions additionally follow the table-owned ID
rule: the row data no longer stores its own ID.

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

**Modeling-label and provenance fields** (moved off the constraint struct entirely; query the
host's per-collection context store):
```rust,ignore
// ❌ Before (v2 — per-constraint inline)
constraint.name
constraint.subscripts
constraint.parameters
constraint.description

// ❌ Earlier v3 alpha (briefly: a `metadata` field on the struct) — also gone
constraint.metadata.name

// ✅ After — constraint context lives in the SoA store on the enclosing collection
let store = instance.constraint_context();   // &ConstraintContextStore<ConstraintID>
store.name(id)         // Option<&str>
store.subscripts(id)   // &[i64]
store.parameters(id)   // &FnvHashMap<String, String>
store.description(id)  // Option<&str>
store.provenance(id)   // &[Provenance]

// One-shot owned reconstruction matching the pre-SoA fields
let context: ConstraintContext = store.collect_for(id);
```

The same shape applies to indicator / one-hot / sos1 constraints
(`indicator_constraint_context()`, …) and to decision variables
(`variable_labels()` exposes a `VariableLabelStore` without
`provenance`). For named functions the parallel accessor is
`named_function_labels()` returning a `NamedFunctionLabelStore`.

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

// ✅ After — no `id` and no inline label/context
Constraint {
    equality: Equality::EqualToZero,
    stage: CreatedData { function },
}

// ✅ Factory methods no longer take an ID
Constraint::equal_to_zero(function)
Constraint::less_than_or_equal_to_zero(function)

// ✅ Insertion via the host's invariant-safe entry point — picks an
// unused id, drains the (optional) context into the SoA store,
// validates required_ids, returns the assigned id. `add_constraint`,
// `relax_constraint`, and `restore_constraint` all take `&mut self`,
// so `instance` must be a `mut` binding (or accessed via `&mut Instance`).
let id = instance.add_constraint(
    Constraint::equal_to_zero(function),
    ConstraintContext {
        label: ModelingLabel {
            name: Some("demand_balance".into()),
            ..Default::default()
        },
        ..Default::default()
    },
)?;

// `relax_constraint` / `restore_constraint` move id between active and
// removed; context stays in place. There is no `constraint_collection_mut()`
// — the raw map mutators on `ConstraintCollection<T>` are `pub(crate)`.
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

// ✅ After — no `id`, no inline label/context; insert with the key when
// storing. Context for the id rides on the parent
// `EvaluatedCollection<T>::context` SoA store.
Constraint {
    equality,
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
`decision_variables` and keep the active / removed maps disjoint.
Crate-internal transformation code also routes through operation-level
collection effects rather than raw active / removed / context map mutation.

### 6. getset Removal

`EvaluatedConstraint` and `SampledConstraint` no longer use the `getset` crate. All fields are accessed directly via `self.equality` and `self.stage.*`. (`self.id` and `self.metadata` no longer exist on the struct — see [Modeling labels and constraint context](#modeling-labels-and-constraint-context) and the constraint-field-access section above.)

Methods like `.id()`, `.equality()`, `.evaluated_value()`, `.feasible()` are **removed**. Use field access instead.

### 7. Error Surface Call-Site Rewrites

See the [3.0 release note](crate::doc::release_note::v3_0) for the
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

Generic collection of active + removed constraints, plus the SoA
context store for the kind. Also implements `Evaluate`:

```rust,ignore
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
    context: ConstraintContextStore<T::ID>,
}

// Methods (public)
collection.active()                    // &BTreeMap<T::ID, T::Created>
collection.removed()                   // &BTreeMap<T::ID, (T::Created, RemovedReason)>
collection.context()                   // &ConstraintContextStore<T::ID>
// There is no public split-into-maps operation. Mutation and conversion go
// through Instance / ParametricInstance methods so invariants
// (active/removed disjointness, variable-id validity, and context ownership)
// are enforced. Crate-internal transformations use operation-level row
// effects rather than raw map mutation.

// Evaluate trait impl
collection.evaluate(state, atol)           // EvaluatedCollection<T>
collection.evaluate_samples(samples, atol) // SampledCollection<T>
collection.partial_evaluate(state, atol)   // only active constraints
collection.required_ids()                  // VariableIDSet
```

Removed constraints are just `Created` constraints paired with a `RemovedReason`. The `Removed` stage type no longer exists.

### EvaluatedCollection / SampledCollection

Generic wrappers for evaluation results, used in `Solution` and `SampleSet`. Each carries the same `ConstraintContextStore<T::ID>` as the source `ConstraintCollection<T>` so per-id context is available at every stage:

```rust,ignore
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    context: ConstraintContextStore<T::ID>,
}

pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    context: ConstraintContextStore<T::ID>,
}

// Both Deref to BTreeMap<T::ID, T::Evaluated/Sampled> for backward-compatible access
// and provide feasibility / removal / context accessors:
collection.is_feasible()               // all constraints feasible
collection.is_feasible_relaxed()       // all non-removed constraints feasible
collection.is_removed(&id)             // check if a constraint was removed
collection.removed_reasons()           // &BTreeMap<T::ID, RemovedReason>
collection.context()                   // &ConstraintContextStore<T::ID>
```

### Modeling labels and constraint context

Per-collection Struct-of-Arrays stores replace the inline fields that
used to live on every `Constraint` / `DecisionVariable` /
`NamedFunction`, and on legacy `v1::Parameter` rows. Decision variables,
parameters, and named functions carry `ModelingLabel`; constraints carry
`ConstraintContext`, which contains a `ModelingLabel` plus constraint-only
transformation provenance. Four families:

```rust,ignore
pub struct ConstraintContextStore<ID> { /* label + provenance */ }
pub struct VariableLabelStore       { /* same, no provenance */ }
pub struct ParameterLabelStore      { /* same, no provenance */ }
pub struct NamedFunctionLabelStore  { /* same, no provenance */ }
```

Per-host accessors on `Instance` and `ParametricInstance` give read
access to every store. Label/context writes go through owner-checked setters
so labels/provenance cannot be attached to IDs that the host does not
own:

```rust,ignore
instance.constraint_context()              // &ConstraintContextStore<ConstraintID>
instance.set_constraint_context(id, context)?
instance.indicator_constraint_context()    // &ConstraintContextStore<IndicatorConstraintID>
instance.set_indicator_constraint_context(id, context)?
instance.one_hot_constraint_context()
instance.set_one_hot_constraint_context(id, context)?
instance.sos1_constraint_context()
instance.set_sos1_constraint_context(id, context)?
instance.variable_labels()                // &VariableLabelStore
instance.set_variable_label(id, label)?
parametric.parameters()                   // &ParameterTable
parametric.parameters().labels()          // &ParameterLabelStore
instance.named_function_table()           // &NamedFunctionTable<NamedFunction>
instance.named_function_labels()          // &NamedFunctionLabelStore
instance.set_named_function_label(id, label)?
```

`Solution` and `SampleSet` expose the variable / named-function stores
the same way (`solution.variable_labels()`,
`solution.evaluated_named_function_table()`,
`solution.named_function_labels()`, same on `SampleSet`), but
constraint context is reached through the evaluated / sampled
collection getter then `.context()` on the collection — there are no
flattened `solution.constraint_context()` shortcuts at the host level:

```rust,ignore
solution.evaluated_constraints().context()              // &ConstraintContextStore<ConstraintID>
solution.evaluated_indicator_constraints().context()    // … <IndicatorConstraintID>
solution.evaluated_one_hot_constraints().context()
solution.evaluated_sos1_constraints().context()

sample_set.constraints().context()                      // &ConstraintContextStore<ConstraintID>
sample_set.indicator_constraints().context()
// etc.
```

Store API:

```rust,ignore
impl<ID> ConstraintContextStore<ID> {
    // Per-field borrowing reads. EMPTY_* sentinels cover the absent case
    // so the underlying Option<…> storage doesn't leak through.
    pub fn name(&self, id: ID)        -> Option<&str>;
    pub fn subscripts(&self, id: ID)  -> &[i64];
    pub fn parameters(&self, id: ID)  -> &FnvHashMap<String, String>;
    pub fn description(&self, id: ID) -> Option<&str>;
    pub fn provenance(&self, id: ID)  -> &[Provenance];

    // One-shot owned reconstruction matching the I/O struct.
    pub fn collect_for(&self, id: ID) -> ConstraintContext;

    // Setters (write-through to the SoA store).
    pub fn set_name(&mut self, id: ID, name: impl Into<String>);
    pub fn set_subscripts(&mut self, id: ID, s: impl Into<Vec<i64>>);
    pub fn push_subscript(&mut self, id: ID, value: i64);
    pub fn set_parameter(&mut self, id: ID, key: impl Into<String>, value: impl Into<String>);
    pub fn set_parameters(&mut self, id: ID, params: FnvHashMap<String, String>);
    pub fn set_description(&mut self, id: ID, desc: impl Into<String>);
    pub fn push_provenance(&mut self, id: ID, p: Provenance);
    pub fn set_provenance(&mut self, id: ID, p: Vec<Provenance>);

    // Bulk owned exchange with the I/O struct.
    pub fn insert(&mut self, id: ID, context: ConstraintContext);
    pub fn remove(&mut self, id: ID) -> ConstraintContext;
}
```

`VariableLabelStore`, `ParameterLabelStore`, and
`NamedFunctionLabelStore` mirror the shape above with the provenance fields
omitted (`provenance(id)`, `push_provenance`, `set_provenance`).
`VariableLabelStore` keeps the subscript append helpers (`push_subscript`,
`extend_subscripts`); `NamedFunctionLabelStore` does not — extend a named
function's subscripts via `set_subscripts(id, new_vec)` instead.

### Fixed decision-variable values

Decision-variable IDs and fixed values no longer live on
[`DecisionVariable`](crate::DecisionVariable). The variable struct is now the
row data of the host's decision-variable table and contains only its intrinsic
definition (`kind`, `bound`). The [`VariableID`](crate::VariableID) is owned by
the enclosing table key. Created-stage hosts
([`Instance`](crate::Instance) and
[`ParametricInstance`](crate::ParametricInstance)) store rows, modeling labels,
and fixed values together in
[`DecisionVariableTable`](crate::DecisionVariableTable). The table validates
that labels and fixed values target existing
decision-variable IDs and that fixed values satisfy the row kind/bound.
`DecisionVariableTable` is parameterized by the same shared lifecycle stages as
constraints:
[`EvaluatedDecisionVariableTable`](crate::EvaluatedDecisionVariableTable) and
[`SampledDecisionVariableTable`](crate::SampledDecisionVariableTable) are the
evaluated and sampled stage aliases, sharing the same row-ID and label-owner
invariants while omitting the created-stage fixed-value column.

The row still owns the `kind`/`bound` invariant: `DecisionVariable::new` and
bound mutation normalize `bound` through `kind.consistent_bound(bound, atol)`.
This preserves the main-branch guarantee that a safely constructed
`DecisionVariable` never stores an unnormalized bound for its kind.

Construction signatures changed accordingly:

```rust,ignore
// ❌ Before
let x = DecisionVariable::new(id, kind, bound, Some(value), atol)?;
let y = DecisionVariable::new(id, kind, bound, None, atol)?;
let z = DecisionVariable::binary(id);
dv.substitute(value, atol)?;
let fixed = dv.substituted_value();

let evaluated = EvaluatedDecisionVariable::new(dv, value, atol)?;
let sampled = SampledDecisionVariable::new(dv, samples, atol)?;

// ✅ After
let y = DecisionVariable::new(kind, bound, atol)?;
let z = DecisionVariable::binary();

let instance = Instance::builder()
    .decision_variables(BTreeMap::from([(id, y.clone())]))
    .fixed_decision_variable_values(BTreeMap::from([(id, value)]))
    .build()?;

let fixed = instance.fixed_decision_variable_value(id);
let all_fixed = instance.fixed_decision_variable_values();

let evaluated = EvaluatedDecisionVariable::new(id, y, value)?;
let sampled = SampledDecisionVariable::new(id, y, samples)?;
```

When constructing a created-stage table directly, use
`DecisionVariableTable::with_fixed_values(entries, labels, fixed_values, atol)`.
If no variables are fixed, pass an empty `fixed_values` map; this is the same
table schema with an empty fixed-value column, not a separate construction mode.

`Instance::partial_evaluate` writes new fixed values into the created
decision-variable table.
State entries for keys in `decision_variable_dependency` are treated as
consistency assertions: they are accepted only when the dependency RHS is fully
determined by other fixed/state values and matches within tolerance. For
example, with `y <- 2 * x`, `partial_evaluate({x: 2, y: 4})` is accepted and
normalizes `y` into `fixed_decision_variable_values`, while
`partial_evaluate({y: 4})` is rejected because it would require solving the
dependency backwards.
Legacy v1 protobuf `substituted_value` fields are still accepted on parse, but
the parser drains them into the same table before constructing the domain
model. The host builder rejects states where a fixed variable is also
solver-used or dependent, and `ParametricInstance` additionally rejects
decision-variable / parameter ID collisions; these host-level invariants cannot
be checked by an individual `DecisionVariable` or by the table alone.

`EvaluatedDecisionVariable::new(id, ...)` and
`SampledDecisionVariable::new(id, ...)` accept an ID so non-finite value errors
can still report the table key. The evaluated/sampled row data itself does not
store the ID; `Solution` and `SampleSet` own it through
[`EvaluatedDecisionVariableTable`](crate::EvaluatedDecisionVariableTable) and
[`SampledDecisionVariableTable`](crate::SampledDecisionVariableTable),
respectively.

### Named-function table ownership

Named-function IDs and labels no longer live on
[`NamedFunction`](crate::NamedFunction),
[`EvaluatedNamedFunction`](crate::EvaluatedNamedFunction), or
[`SampledNamedFunction`](crate::SampledNamedFunction). The row structs carry
only intrinsic data:

- `NamedFunction`: the [`Function`](crate::Function)
- `EvaluatedNamedFunction`: the evaluated value and used decision-variable IDs
- `SampledNamedFunction`: sampled values and used decision-variable IDs

The [`NamedFunctionID`](crate::NamedFunctionID) and modeling labels are owned by
[`NamedFunctionTable`](crate::NamedFunctionTable). `Instance` and
`ParametricInstance` store `NamedFunctionTable<NamedFunction>`, `Solution`
stores `NamedFunctionTable<EvaluatedNamedFunction>`, and `SampleSet` stores
`NamedFunctionTable<SampledNamedFunction>`. The table keeps row payloads and
[`NamedFunctionLabelStore`](crate::NamedFunctionLabelStore) together so labels
cannot be attached to unknown named-function IDs at validated construction
boundaries.

Host accessors expose shared table views only. They intentionally do not expose
mutable row views because changing a named-function body after host validation
could introduce undefined variable IDs. Add named functions through
[`Instance::new_named_function`](crate::Instance::new_named_function) or the
validated builders; `new_named_function` returns the allocated
[`NamedFunctionID`](crate::NamedFunctionID), not a mutable row reference.

Construction changes mirror constraints and decision variables:

```rust,ignore
// Before
let nf = NamedFunction {
    id,
    function,
};
let evaluated = named_function.evaluate(&state, atol)?;
let evaluated_id = evaluated.id();

// After
let nf = NamedFunction { function };
let instance = Instance::builder()
    .named_functions(BTreeMap::from([(id, nf.clone())]))
    .build()?;

let evaluated = nf.evaluate(&state, atol)?;
let solution = Solution::builder()
    .evaluated_named_functions(BTreeMap::from([(id, evaluated)]))
    .build()?;
```

The deprecated `Solution::new(...)` constructor was removed because it was a
safe API that skipped host-level validation. Construct solutions through
`Solution::builder().build()?`; reserve `build_unchecked` for code paths where
the enclosing owner has already guaranteed all `Solution` invariants.

Legacy `ommx.v1` protobuf messages still carry an inline `id` field. Rust parse
drains that field into the owning map key, and Rust serialization fills it from
the map key; the domain row remains ID-less on both sides of the conversion.

### Parameter table ownership

`ParametricInstance` parameters now follow the same owner-boundary rule, but
with one important difference: parameters intentionally do **not** get a
separate `ParameterID` type. Parameter references share the
[`VariableID`](crate::VariableID) namespace with decision variables because a
[`Function`](crate::Function) only carries variable IDs; only the enclosing
[`ParametricInstance`](crate::ParametricInstance) can decide whether an ID is a
decision variable or a parameter.

The Rust domain model therefore stores parameters as a
[`ParameterTable`](crate::ParameterTable):

- [`ParameterTable`](crate::ParameterTable) owns the parameter ID set and the
  [`ParameterLabelStore`](crate::ParameterLabelStore).
- The table-level invariant is that label IDs are a subset of parameter IDs.
- [`ParametricInstance`](crate::ParametricInstance) owns the host-level
  invariants: parameter IDs and decision-variable IDs are disjoint, expression
  bodies reference IDs from their union, and structural decision-variable
  positions such as indicator / one-hot / SOS1 members never use parameter IDs.
- Parameter values are not table data. They are supplied later through
  [`ParametricInstance::with_parameters`](crate::ParametricInstance::with_parameters).

This removes the former `BTreeMap<VariableID, v1::Parameter>` duplication where
the map key and `v1::Parameter.id` both claimed to own the same ID. Legacy
`ommx.v1.Parameter` protobuf rows are still parsed and written at the
serialization boundary, but their inline IDs and labels are drained into /
filled from the `ParameterTable`.

```rust,ignore
// Before
let parameters = BTreeMap::from([(
    id,
    v1::Parameter {
        id: id.into_inner(),
        name: Some("p".to_string()),
        ..Default::default()
    },
)]);
let pi = ParametricInstance::builder()
    .parameters(parameters)
    .build()?;

// After
let mut labels = ParameterLabelStore::default();
labels.set_name(id, "p");
let parameters = ParameterTable::new(BTreeSet::from([id]), labels)?;
let pi = ParametricInstance::builder()
    .parameters(parameters)
    .build()?;
```

### ConstraintContext

Owned struct used as the I/O type for constraint context (insertion via
`add_constraint(c, context)`, owned reads via `store.collect_for(id)`,
modeling-chain staging on the Python `Constraint` snapshot wrapper).
The modeling label is nested so provenance remains separate:

```rust,ignore
pub struct ConstraintContext {
    pub label: ModelingLabel,
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
- [ ] Update `constraint.name` reads — context is no longer on the constraint struct. Query the host's SoA store: `instance.constraint_context().name(id)` (and `subscripts`, `parameters`, `description`, `provenance`); use `collect_for(id) -> ConstraintContext` for an owned snapshot.
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
- [ ] Replace `DecisionVariable::new(id, kind, bound, ..., atol)` with `DecisionVariable::new(kind, bound, atol)`, and insert it under the desired `VariableID` key in the host table
- [ ] Replace `DecisionVariable::binary(id)` / `integer(id)` / `continuous(id)` / etc. with the no-argument row factories, and keep the ID on the enclosing map key
- [ ] Replace `DecisionVariable::substituted_value()` and `DecisionVariable::substitute(...)` with host-owned fixed values: `InstanceBuilder::fixed_decision_variable_values(...)`, `Instance::fixed_decision_variable_value(id)`, or `Instance::fixed_decision_variable_values()`
- [ ] Update `EvaluatedDecisionVariable::new(...)` and `SampledDecisionVariable::new(...)`: drop the `atol` argument, pass the `VariableID` separately for diagnostics, and keep using the enclosing `Solution` / `SampleSet` map key as the source of truth
- [ ] Remove `NamedFunction.id`, `EvaluatedNamedFunction::id()`, and `SampledNamedFunction::id()` reads in Rust. Use the enclosing `NamedFunctionTable<_>` key instead
- [ ] Construct `NamedFunction { function }` rows and insert them under the desired `NamedFunctionID` key; keep row maps and labels together with `NamedFunctionTable`
- [ ] Update `Instance::new_named_function(...)` callers to use the returned `NamedFunctionID`; it no longer returns `&mut NamedFunction`
- [ ] Replace `BTreeMap<VariableID, v1::Parameter>` on Rust `ParametricInstance` builders / constructors with `ParameterTable`; keep parameter IDs as `VariableID` keys, not a separate `ParameterID`
- [ ] Move parameter `name` / `subscripts` / `parameters` / `description` access to `parametric.parameters().labels()`, and keep concrete parameter values in `ParametricInstance::with_parameters(...)`

---

# Rust SDK v3 Artifact API Migration Guide

This section covers the Artifact / Local Registry API changes for users
moving from `ommx` v2 to v3. The v3 Local Registry is SQLite-backed
(IndexStore + filesystem CAS BlobStore) rather than an on-disk OCI
Image Layout per `image:tag`.

## Overview

Artifact construction was a single generic `Builder<Base: ImageBuilder>`
that switched between `.ommx` archive output and a legacy on-disk
"OCI Image Layout" local registry depending on the `Base` type. v3
collapses that split: every commit goes through `ArtifactDraft`
and lands in the SQLite Local Registry. A `.ommx` file is just an
exchange-format export of a registry-resident artifact, produced by
`LocalArtifact::save(path)`.

| v2 | v3 |
|---|---|
| `Builder<OciDirBuilder>` (local registry) | `ArtifactDraft` |
| `Builder<OciArchiveBuilder>` (`.ommx` file) | `ArtifactDraft::new(...).commit()?.save(path)?` |

The local-registry path now writes an OCI Image Manifest (per OCI 1.1
spec, with `artifactType`) into a SQLite-backed registry instead of an
on-disk OCI Image Layout directory. Existing legacy
`<root>/<image>/<tag>/` directories are identity-preserved on import
via `ommx import-legacy` or the `import_legacy_local_registry*` SDK
functions — pulled bytes (manifest digest and JSON) round-trip verbatim.

## Breaking Changes

### 1. Local Registry draft

```rust,ignore
// ❌ Before
use ommx::artifact::Builder;
let mut builder = Builder::for_github("Jij-Inc", "demo", "experiment", "v1")?;
builder.add_instance(instance, annotations)?;
let artifact = builder.build()?;

// ✅ After
use ommx::artifact::ArtifactDraft;
let mut draft = ArtifactDraft::for_github("Jij-Inc", "demo", "experiment", "v1")?;
draft.add_instance(instance)?;
let artifact = draft.commit()?;
```

`Builder<OciDirBuilder>::{new, for_github}` are removed. Use
`ArtifactDraft::{new, for_github}` instead. Output lands in the
v3 SQLite registry rather than the legacy `<root>/<image>/<tag>/`
OCI Image Layout directory.

### 2. Archive output goes through ArtifactDraft

```rust,ignore
// ❌ Before
use ommx::artifact::Builder;
let mut builder = Builder::new_archive(path, image_name)?;
builder.add_instance(instance, ann)?;
let artifact = builder.build()?;

// ✅ After
use ommx::artifact::ArtifactDraft;
let mut draft = ArtifactDraft::new(image_name)?;
draft.add_instance(instance)?;
let artifact = draft.commit()?;
artifact.save(&path)?;
```

`ArchiveArtifactBuilder` is gone. The same `ArtifactDraft`
publishes into the SQLite Local Registry, and `LocalArtifact::save`
exports a `.ommx` file. Constructors:

- `ArtifactDraft::new(image_name)?` — caller-supplied ref name.
- `ArtifactDraft::new_anonymous()?` — constructs
  `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>`
  against the default registry's `registry_id` (a random UUID
  generated once per `LocalRegistry` and persisted in SQLite
  metadata). The local-time `YYYYMMDDTHHMMSS` prefix lets you read
  the creation time at a glance, and the 12-hex (48-bit) random nonce
  keeps concurrent / scripted anonymous commits collision-free
  regardless of the host's clock resolution. The `.local` mDNS TLD
  prevents an accidental push from leaking to a real remote registry.
  `ommx prune-anonymous` bulk-cleans every registry-id
  prefix's anonymous refs.
- `ArtifactDraft::temp()` — random `ttl.sh/<uuid>:1h` name;
  insecure, tests only.
- `ArtifactDraft::for_github(org, repo, name, tag)` — GHCR
  helper.

`commit()` returns `LocalArtifact`. The `add_*` signatures are
`add_layer_bytes` / `add_instance` / `add_solution` /
`add_parametric_instance` / `add_sample_set`.

## Migration Checklist

- [ ] Replace `ommx::artifact::Builder` (both `OciDirBuilder` and
      `OciArchiveBuilder` variants) with `ArtifactDraft`.
- [ ] Replace `Builder::new_archive(path, name)` + `.build()` with
      `ArtifactDraft::new(name)?.commit()?.save(&path)?`.
- [ ] Replace `Builder::new_archive_unnamed(path)` with
      `ArtifactDraft::new_anonymous()?.commit()?.save(&path)?`.

- [ ] Replace `Builder::for_github` with `ArtifactDraft::for_github`.
- [ ] Replace `temp_archive()` with `ArtifactDraft::temp()?.commit()?.save(&path)?`.
- [ ] Replace `ocipkg::ImageName` with `ommx::artifact::ImageRef`. The
      type is a newtype around `oci_spec::distribution::Reference`,
      so the full distribution-reference grammar applies. It accepts
      `host[:port]/name:tag`, `host[:port]/name@<digest>`, and the
      combined `tag@<digest>` form on parse, and canonicalises digest
      references to `name@<digest>` on `Display` (tag references keep
      `:`). The accessor shape is `registry()` (the joined
      `host[:port]` form, same as
      `oci_spec::distribution::Reference::registry`) plus `name()` /
      `reference()`. The v2 split accessors `hostname` /
      `port` are **gone**: every internal consumer ended up
      rejoining them at the call site, so the wrapper now exposes
      the joined form directly. Callers that genuinely need just the
      host portion (e.g. a localhost / 127.* heuristic) should parse
      `registry()` inline. Bare-namespace inputs without an
      explicit registry (`library/ubuntu:20.04`, `alpine`) default to
      `docker.io` via the standard Docker reference heuristic — the
      first segment is only treated as a host when it contains `.`
      or `:` or equals `localhost`. The `ommx::ocipkg` re-export is
      removed in v3, so any direct `use ommx::ocipkg::ImageName` call
      site needs to switch.
- [ ] Be aware of the **Docker Hub hostname canonicalisation** when
      sharing user-data caches across SDK versions. SDK v2's `ocipkg`
      defaulted bare image names to the hostname
      `registry-1.docker.io`; v3 normalises to the OCI canonical
      `docker.io` with `library/` prefix added for single-segment
      names. `ImageRef::parse` includes a one-line shim that rewrites
      `registry-1.docker.io/` to `docker.io/` so v2 archive
      annotations and disk-cache layouts collapse onto the same SQLite
      key that `Artifact.load("alpine")` queries. `ocipkg`'s legacy
      digest spelling `name:algorithm:hex` is **not** accepted by
      `oci_spec` and is not back-translated — digest-pinned v2
      annotations had to already use the OCI-standard `name@<digest>`
      form (which is what ocipkg's archive writer emitted in
      practice).
- [ ] Drop calls to `ommx::artifact::get_image_dir` /
      `ommx::artifact::image_dir`. These returned a v2 disk-cache
      path (`<root>/<image_name>/<tag>/`) that no longer corresponds
      to anything in the v3 SQLite Local Registry. The v2 → v3
      migration check that previously read this path moves to
      `ommx::artifact::local_registry::LocalRegistry::legacy_ref_path_in`,
      which is the public compatibility entry point that still computes
      the v2-shaped path for migration checks.
      The `ommx image-dir <name>` CLI subcommand and the Python
      `ommx.get_image_dir` function are removed for the same
      reason — pointing users at a path that is unrelated to v3
      storage was actively misleading.
