# Rust SDK Migration Guide

This document covers migration of the OMMX Rust SDK (`ommx` crate) across major versions.

- [v3 (Stage Pattern)](#rust-sdk-v3-stage-pattern-migration-guide) — Constraint lifecycle stage parameterization
- [v3 (Artifact API)](#rust-sdk-v3-artifact-api-migration-guide) — Local registry / archive builder split and renames

---

# Rust SDK v3 Stage Pattern Migration Guide

This section covers the migration to stage-parameterized constraints
landed in `3.0.0-alpha.1`.

## Overview

`Constraint` is now generic over a lifecycle stage, its `ConstraintID`
lives on the enclosing collection key rather than on the struct itself,
and metadata (`name`, `subscripts`, `parameters`, `description`,
`provenance`) lives in a Struct-of-Arrays store on the enclosing
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
| `SampledConstraint` | `Constraint<stage::Sampled>` | `SampledData { evaluated_values, feasible, ... }` |

Removed constraints are managed at the collection level —
`ConstraintCollection` stores them as `(Constraint<Created>, RemovedReason)`
pairs. "Removed" is not itself a stage.

`DecisionVariable`, `IndicatorConstraint<S>`, `OneHotConstraint<S>`,
`Sos1Constraint<S>`, and `NamedFunction` got the same SoA treatment —
each lost its inline metadata fields, and the per-host metadata store
is queried through narrow per-collection accessors on `Instance` /
`ParametricInstance` (see [Metadata stores](#metadata-stores)).

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

**Metadata fields** (moved off the constraint struct entirely; query the
host's per-collection metadata store):
```rust,ignore
// ❌ Before (v2 — per-constraint inline)
constraint.name
constraint.subscripts
constraint.parameters
constraint.description

// ❌ Earlier v3 alpha (briefly: a `metadata` field on the struct) — also gone
constraint.metadata.name

// ✅ After — metadata lives in the SoA store on the enclosing collection
let store = instance.constraint_metadata();   // &ConstraintMetadataStore<ConstraintID>
store.name(id)         // Option<&str>
store.subscripts(id)   // &[i64]
store.parameters(id)   // &FnvHashMap<String, String>
store.description(id)  // Option<&str>
store.provenance(id)   // &[Provenance]

// One-shot owned reconstruction matching the pre-SoA struct
let metadata: ConstraintMetadata = store.collect_for(id);
```

The same shape applies to indicator / one-hot / sos1 constraints
(`indicator_constraint_metadata()`, …) and to decision variables
(`variable_metadata()` exposes a `VariableMetadataStore` without
`provenance`). For named functions the parallel accessor is
`named_function_metadata()` returning a `NamedFunctionMetadataStore`.

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

// ✅ After — no `id` and no inline metadata
Constraint {
    equality: Equality::EqualToZero,
    stage: CreatedData { function },
}

// ✅ Factory methods no longer take an ID
Constraint::equal_to_zero(function)
Constraint::less_than_or_equal_to_zero(function)

// ✅ Insertion via the host's invariant-safe entry point — picks an
// unused id, drains the (optional) metadata into the SoA store,
// validates required_ids, returns the assigned id. `add_constraint`,
// `relax_constraint`, and `restore_constraint` all take `&mut self`,
// so `instance` must be a `mut` binding (or accessed via `&mut Instance`).
let id = instance.add_constraint(
    Constraint::equal_to_zero(function),
    ConstraintMetadata { name: Some("demand_balance".into()), ..Default::default() },
)?;

// `relax_constraint` / `restore_constraint` move id between active and
// removed; metadata stays in place. There is no `constraint_collection_mut()`
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

// ✅ After — no `id`, no inline metadata; insert with the key when
// storing. Metadata for the id rides on the parent
// `EvaluatedCollection<T>::metadata` SoA store.
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
`decision_variables` and keep the active / removed maps disjoint. The
raw `active_mut()` / `removed_mut()` mutators on
`ConstraintCollection<T>` are `pub(crate)` and not callable from
outside the crate.

### 6. getset Removal

`EvaluatedConstraint` and `SampledConstraint` no longer use the `getset` crate. All fields are accessed directly via `self.equality` and `self.stage.*`. (`self.id` and `self.metadata` no longer exist on the struct — see [Metadata stores](#metadata-stores) and the constraint-field-access section above.)

Methods like `.id()`, `.equality()`, `.evaluated_value()`, `.feasible()` are **removed**. Use field access instead.

### 7. Error Surface Call-Site Rewrites

See the [release note](crate::doc::release_note) for the
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
metadata store for the kind. Also implements `Evaluate`:

```rust,ignore
pub struct ConstraintCollection<T: ConstraintType> {
    active: BTreeMap<T::ID, T::Created>,
    removed: BTreeMap<T::ID, (T::Created, RemovedReason)>,
    metadata: ConstraintMetadataStore<T::ID>,
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

Generic wrappers for evaluation results, used in `Solution` and `SampleSet`. Each carries the same `ConstraintMetadataStore<T::ID>` as the source `ConstraintCollection<T>` so per-id metadata is available at every stage:

```rust,ignore
pub struct EvaluatedCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    metadata: ConstraintMetadataStore<T::ID>,
}

pub struct SampledCollection<T: ConstraintType> {
    constraints: BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    metadata: ConstraintMetadataStore<T::ID>,
}

// Both Deref to BTreeMap<T::ID, T::Evaluated/Sampled> for backward-compatible access
// and provide feasibility / removal / metadata accessors:
collection.is_feasible()               // all constraints feasible
collection.is_feasible_relaxed()       // all non-removed constraints feasible
collection.is_removed(&id)             // check if a constraint was removed
collection.removed_reasons()           // &BTreeMap<T::ID, RemovedReason>
collection.metadata()                  // &ConstraintMetadataStore<T::ID>
```

### Metadata stores

Per-collection Struct-of-Arrays metadata stores replace the inline
metadata fields that used to live on every `Constraint` /
`DecisionVariable` / `NamedFunction`. Three families:

```rust,ignore
pub struct ConstraintMetadataStore<ID> { /* name / subscripts / parameters / description / provenance */ }
pub struct VariableMetadataStore       { /* same, no provenance */ }
pub struct NamedFunctionMetadataStore  { /* same, no provenance */ }
```

Per-host accessors on `Instance` and `ParametricInstance` give direct
read / write access to every store:

```rust,ignore
instance.constraint_metadata()              // &ConstraintMetadataStore<ConstraintID>
instance.constraint_metadata_mut()          // &mut …
instance.indicator_constraint_metadata()    // &ConstraintMetadataStore<IndicatorConstraintID>
instance.indicator_constraint_metadata_mut()
instance.one_hot_constraint_metadata() / _mut()
instance.sos1_constraint_metadata()    / _mut()
instance.variable_metadata()           / _mut()        // &VariableMetadataStore
instance.named_function_metadata()     / _mut()        // &NamedFunctionMetadataStore
```

`Solution` and `SampleSet` expose the variable / named-function stores
the same way (`solution.variable_metadata()`,
`solution.named_function_metadata()`, same on `SampleSet`), but
constraint metadata is reached through the evaluated / sampled
collection getter then `.metadata()` on the collection — there are no
flattened `solution.constraint_metadata()` shortcuts at the host level:

```rust,ignore
solution.evaluated_constraints().metadata()              // &ConstraintMetadataStore<ConstraintID>
solution.evaluated_indicator_constraints().metadata()    // … <IndicatorConstraintID>
solution.evaluated_one_hot_constraints().metadata()
solution.evaluated_sos1_constraints().metadata()

sample_set.constraints().metadata()                      // &ConstraintMetadataStore<ConstraintID>
sample_set.indicator_constraints().metadata()
// etc.
```

Store API:

```rust,ignore
impl<ID> ConstraintMetadataStore<ID> {
    // Per-field borrowing reads. EMPTY_* sentinels cover the absent case
    // so the underlying Option<…> storage doesn't leak through.
    pub fn name(&self, id: ID)        -> Option<&str>;
    pub fn subscripts(&self, id: ID)  -> &[i64];
    pub fn parameters(&self, id: ID)  -> &FnvHashMap<String, String>;
    pub fn description(&self, id: ID) -> Option<&str>;
    pub fn provenance(&self, id: ID)  -> &[Provenance];

    // One-shot owned reconstruction matching the I/O struct.
    pub fn collect_for(&self, id: ID) -> ConstraintMetadata;

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
    pub fn insert(&mut self, id: ID, metadata: ConstraintMetadata);
    pub fn remove(&mut self, id: ID) -> ConstraintMetadata;
}
```

`VariableMetadataStore` and `NamedFunctionMetadataStore` mirror the
shape above with the provenance fields omitted (`provenance(id)`,
`push_provenance`, `set_provenance`). `VariableMetadataStore` keeps the
subscript append helpers (`push_subscript`, `extend_subscripts`);
`NamedFunctionMetadataStore` does not — extend a named function's
subscripts via `set_subscripts(id, new_vec)` instead.

### ConstraintMetadata

Owned struct used as the I/O type for metadata (insertion via
`add_constraint(c, metadata)`, owned reads via `store.collect_for(id)`,
modeling-chain staging on the Python `Constraint` snapshot wrapper).
Same shape as the pre-SoA struct:

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
- [ ] Update `constraint.name` reads — metadata is no longer on the constraint struct. Query the host's SoA store: `instance.constraint_metadata().name(id)` (and `subscripts`, `parameters`, `description`, `provenance`); use `collect_for(id) -> ConstraintMetadata` for an owned snapshot.
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
collapses that split: every build goes through `LocalArtifactBuilder`
and lands in the SQLite Local Registry. A `.ommx` file is just an
exchange-format export of a registry-resident artifact, produced by
`LocalArtifact::save(path)`.

| v2 | v3 |
|---|---|
| `Builder<OciDirBuilder>` (local registry) | `LocalArtifactBuilder` |
| `Builder<OciArchiveBuilder>` (`.ommx` file) | `LocalArtifactBuilder::new(...).build()?.save(path)?` |

The local-registry path now writes an OCI Image Manifest (per OCI 1.1
spec, with `artifactType`) into a SQLite-backed registry instead of an
on-disk OCI Image Layout directory. Existing legacy
`<root>/<image>/<tag>/` directories are identity-preserved on import
via `ommx artifact import` or the `import_legacy_local_registry*` SDK
functions — pulled bytes (manifest digest and JSON) round-trip verbatim.

## Breaking Changes

### 1. Local Registry builder

```rust,ignore
// ❌ Before
use ommx::artifact::Builder;
let mut builder = Builder::for_github("Jij-Inc", "demo", "experiment", "v1")?;
builder.add_instance(instance, annotations)?;
let artifact = builder.build()?;

// ✅ After
use ommx::artifact::LocalArtifactBuilder;
let mut builder = LocalArtifactBuilder::for_github("Jij-Inc", "demo", "experiment", "v1")?;
builder.add_instance(instance, annotations)?;
let artifact = builder.build()?;
```

`Builder<OciDirBuilder>::{new, for_github}` are removed. Use
`LocalArtifactBuilder::{new, for_github}` instead. Output lands in the
v3 SQLite registry rather than the legacy `<root>/<image>/<tag>/`
OCI Image Layout directory.

### 2. Archive output goes through LocalArtifactBuilder

```rust,ignore
// ❌ Before
use ommx::artifact::Builder;
let mut builder = Builder::new_archive(path, image_name)?;
builder.add_instance(instance, ann)?;
let artifact = builder.build()?;

// ✅ After
use ommx::artifact::LocalArtifactBuilder;
let mut builder = LocalArtifactBuilder::new(image_name);
builder.add_instance(instance, ann)?;
let artifact = builder.build()?;
artifact.save(&path)?;
```

`ArchiveArtifactBuilder` is gone. The same `LocalArtifactBuilder`
publishes into the SQLite Local Registry, and `LocalArtifact::save`
exports a `.ommx` file. Constructors:

- `LocalArtifactBuilder::new(image_name)` — caller-supplied ref name.
- `LocalArtifactBuilder::new_anonymous()` — defers name synthesis to
  `build_in_registry`, which constructs
  `<registry-id8>.ommx.local/anonymous:<local-timestamp>-<nonce>`
  against the destination registry's `registry_id` (a random UUID
  generated once per `LocalRegistry` and persisted in SQLite
  metadata). The local-time `YYYYMMDDTHHMMSS` prefix lets you read
  the creation time at a glance, and the 12-hex (48-bit) random nonce
  keeps concurrent / scripted anonymous builds collision-free
  regardless of the host's clock resolution. The `.local` mDNS TLD
  prevents an accidental push from leaking to a real remote registry.
  `ommx artifact prune-anonymous` bulk-cleans every registry-id
  prefix's anonymous refs.
- `LocalArtifactBuilder::temp()` — random `ttl.sh/<uuid>:1h` name;
  insecure, tests only.
- `LocalArtifactBuilder::for_github(org, repo, name, tag)` — GHCR
  helper.

`build()` returns `LocalArtifact`. The `add_*` signatures are
`add_layer_bytes` / `add_instance` / `add_solution` /
`add_parametric_instance` / `add_sample_set`.

## Migration Checklist

- [ ] Replace `ommx::artifact::Builder` (both `OciDirBuilder` and
      `OciArchiveBuilder` variants) with `LocalArtifactBuilder`.
- [ ] Replace `Builder::new_archive(path, name)` + `.build()` with
      `LocalArtifactBuilder::new(name).build()?.save(&path)?`.
- [ ] Replace `Builder::new_archive_unnamed(path)` with
      `LocalArtifactBuilder::new_anonymous().build()?.save(&path)?`.
      (`new_anonymous()` returns `Self`, not `Result`, so no `?` on
      that call; `build()` materialises the anonymous name against
      the default registry's `registry_id`.)
- [ ] Replace `Builder::for_github` with `LocalArtifactBuilder::for_github`.
- [ ] Replace `temp_archive()` with `LocalArtifactBuilder::temp()?.build()?.save(&path)?`.
- [ ] Replace `ocipkg::ImageName` with `ommx::artifact::ImageRef`. The
      type exposes the same `host[:port]/name:reference` parsing and
      `hostname()` / `port()` / `name()` / `reference()` / `as_path()`
      accessors; field access (`image_name.hostname`, `image_name.port`,
      …) becomes a method call. The `ommx::ocipkg` re-export is
      removed in v3, so any direct `use ommx::ocipkg::ImageName` call
      site needs to switch.
