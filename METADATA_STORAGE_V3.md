# Metadata Storage in OMMX v3 — Design Proposal

Status: **Draft / WIP**

Companion to `SPECIAL_CONSTRAINTS_V3.md`. That doc reshapes the proto schema for
constraint types; this doc reshapes how *metadata* (`name`, `subscripts`,
`parameters`, `description`, `provenance`) is stored at runtime and exposed to
users.

## Goal

Today, `DecisionVariable` and every `Constraint<S>` (regular / indicator /
one-hot / sos1, in all three lifecycle stages) carry an inline `metadata`
field. This metadata is rarely consulted by internal logic — `evaluate`,
`parse`, `substitute`, etc. never read it — but it is copied around on every
clone, serialized on every write, and duplicated wholesale into every wide
`*_df` API on the Python side.

We want to:

1. **Move metadata out of the per-object structs and into ID-keyed columnar
   stores** owned by the enclosing collection (`ConstraintCollection<T>` for
   constraints, `Instance` for decision variables). Internal types lose their
   `metadata` field.
2. **Replace the wide-DataFrame Python API with a JOIN-based one.** Each `*_df`
   returns only the type-specific core columns (`id`, `kind`, `bound`,
   `equality`, `evaluated_value`, …). Metadata, parameters, and provenance live
   in their own thin DataFrames that the user joins on `id` when needed.

These two changes reinforce each other: once the Python API stops embedding
metadata in every wide DataFrame, the Rust-side stores can be exposed
column-for-column without per-row repackaging.

## Why now

- Metadata is on the hot path for memory (`logical_memory.rs` reports the
  per-row `Option`/`Vec`/`FnvHashMap` headers showing up under
  `Instance.constraint_collection;constraints;Constraint.metadata;…` and the
  same under `decision_variables`). Pulling it out lets the per-object struct
  shrink.
- The v3 alpha window is the right moment to break the wide-DataFrame Python
  API. Doing it after v3 GA would require another major.
- `SPECIAL_CONSTRAINTS_V3.md` already extracts `ConstraintMetadata` as a shared
  proto sub-message, so the proto side is already lined up for the runtime
  refactor.

## SDK-level design split

Rust SDK and Python SDK have different requirements; we keep the designs
separate on purpose.

### Rust SDK

Internal users (algorithms, solver adapters, serialization) almost never
look up metadata by ID. The dominant operations are bulk copy and bulk
serialize. Partial updates (`add_decision_variable` one at a time) happen
through normal modeling APIs.

- Internal representation: **Struct-of-Arrays (SoA)**, `FnvHashMap<ID, T>` per
  field. No `polars` dependency.
- The 4 constraint types share the same metadata shape, so the store is one
  generic type parameterized only by the ID type.
- `DecisionVariable` and `Constraint<S>` lose their `metadata` field.

### Python SDK

End users analyze problems and solutions in DataFrames. They want to filter by
`name`, group by `subscripts[0]`, and pivot on `parameters.{key}`.

- Public API: pandas DataFrames with **JOIN-based composition**. Core df, metadata
  df, parameters df, provenance df — separate, each thin, joined on `id`.
- Implementation: bulk-construct each DataFrame from the Rust SoA store. The
  `parameters.{key}` wide-column pattern goes away; `parameters` becomes a
  long-format DataFrame.

### Proto schema

`SPECIAL_CONSTRAINTS_V3.md` already keeps `ConstraintMetadata` inline inside
each constraint message. We retain that. Wire format is AoS; runtime
representation is SoA; the conversion happens at parse / serialize time only.
Hoisting metadata to a top-level `map<uint64, ConstraintMetadata>` would
mirror the runtime layout but adds 4 additional top-level fields per stage and
breaks the inline-readable shape — not worth it.

## Rust SDK design

### Metadata stores

```rust
// Generic over ID type so all 4 constraint types share one implementation.
pub struct ConstraintMetadataStore<ID> {
    name:        FnvHashMap<ID, String>,                         // missing = None
    subscripts:  FnvHashMap<ID, Vec<i64>>,                       // missing = empty
    parameters:  FnvHashMap<ID, FnvHashMap<String, String>>,     // missing = empty
    description: FnvHashMap<ID, String>,                         // missing = None
    provenance:  FnvHashMap<ID, Vec<Provenance>>,                // missing = empty
}

pub struct VariableMetadataStore {
    name:        FnvHashMap<VariableID, String>,
    subscripts:  FnvHashMap<VariableID, Vec<i64>>,
    parameters:  FnvHashMap<VariableID, FnvHashMap<String, String>>,
    description: FnvHashMap<VariableID, String>,
    // no provenance for variables
}
```

`provenance` lives only on constraints; the variable store omits it. Otherwise
the two stores are structurally identical.

### Where the stores live

```rust
pub struct ConstraintCollection<T: ConstraintType> {
    active:   BTreeMap<T::ID, T::Created>,
    removed:  BTreeMap<T::ID, (T::Created, RemovedReason)>,
    metadata: ConstraintMetadataStore<T::ID>,   // new
}

pub struct EvaluatedCollection<T: ConstraintType> {
    constraints:     BTreeMap<T::ID, T::Evaluated>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    metadata:        ConstraintMetadataStore<T::ID>,   // new — copied from parent collection
}

pub struct SampledCollection<T: ConstraintType> {
    constraints:     BTreeMap<T::ID, T::Sampled>,
    removed_reasons: BTreeMap<T::ID, RemovedReason>,
    metadata:        ConstraintMetadataStore<T::ID>,   // new
}

pub struct Instance {
    // existing fields …
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    constraint_collection:           ConstraintCollection<Constraint>,
    indicator_constraint_collection: ConstraintCollection<IndicatorConstraint>,
    one_hot_constraint_collection:   ConstraintCollection<OneHotConstraint>,
    sos1_constraint_collection:      ConstraintCollection<Sos1Constraint>,

    variable_metadata: VariableMetadataStore,   // new
}
```

Why these levels:

- For constraints, `ConstraintCollection<T>` already owns active/removed split
  and is generic over constraint type — putting the store there keeps the
  `relax` / `restore` pair touch-free (active ↔ removed transitions don't move
  metadata at all). The same store rides through to `EvaluatedCollection<T>` /
  `SampledCollection<T>` on the Solution / SampleSet side.
- For variables, there is no analogous `DecisionVariableCollection` and adding
  one only to host metadata would be over-engineering — `Instance` already
  owns `BTreeMap<VariableID, DecisionVariable>` directly. We just add a
  sibling field.

### Per-object struct changes

```rust
pub struct DecisionVariable {
    id:                VariableID,
    kind:              Kind,
    bound:             Bound,
    substituted_value: Option<f64>,
    // metadata field REMOVED
}

pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub stage:    S::Data,
    // metadata field REMOVED
}

// IndicatorConstraint, OneHotConstraint, Sos1Constraint — same: metadata removed.
```

Standalone constraints (created via `Constraint::equal_to_zero(f)`,
`OneHotConstraint::new(...)`, etc.) carry no metadata. Metadata is set after
inserting into a collection:

```rust
let id = collection.insert(Constraint::equal_to_zero(f));
collection.metadata_mut().set_name(id, "demand_balance");
collection.metadata_mut().push_subscripts(id, [i, j]);
```

A builder-style helper can sugar this when convenient:

```rust
collection.insert_with(
    Constraint::equal_to_zero(f),
    |m| m.name("demand_balance").subscripts([i, j]),
);
```

### Access patterns

```rust
impl<T: ConstraintType> ConstraintCollection<T> {
    pub fn metadata(&self) -> &ConstraintMetadataStore<T::ID> { ... }
    pub fn metadata_mut(&mut self) -> &mut ConstraintMetadataStore<T::ID> { ... }

    /// Convenience view bundling constraint + metadata for a single ID.
    pub fn view(&self, id: T::ID) -> Option<ConstraintView<'_, T>> { ... }
}

pub struct ConstraintView<'a, T: ConstraintType> {
    pub id: T::ID,
    pub constraint: &'a T::Created,
    pub metadata: ConstraintMetadataView<'a>,
}

pub struct ConstraintMetadataView<'a> {
    name:        Option<&'a str>,
    subscripts:  &'a [i64],         // empty slice if absent
    parameters:  &'a FnvHashMap<String, String>,   // &EMPTY if absent
    description: Option<&'a str>,
    provenance:  &'a [Provenance],
}
```

`ConstraintMetadataView::parameters()` returns `&EMPTY_MAP` for absent keys
(static const) so the `Option<FnvHashMap<…>>` storage doesn't leak through the
public API.

### Conversion to / from `v1::*` proto types

Currently `From<(ConstraintID, EvaluatedConstraint)> for v1::EvaluatedConstraint`
reads `c.metadata.*` directly. After the refactor:

```rust
impl<T: ConstraintType> EvaluatedCollection<T> {
    pub fn into_v1(self) -> Vec<v1::EvaluatedConstraint> {
        let (constraints, removed) = self.into_parts();
        constraints.into_iter().map(|(id, c)| {
            let metadata = self.metadata.get(id);  // ConstraintMetadataView
            v1::EvaluatedConstraint {
                id: id.into_inner(),
                equality: c.equality.into(),
                evaluated_value: c.stage.evaluated_value,
                name: metadata.name.map(str::to_owned),
                subscripts: metadata.subscripts.to_vec(),
                parameters: metadata.parameters.clone().into_iter().collect(),
                description: metadata.description.map(str::to_owned),
                // …
            }
        }).collect()
    }
}
```

Conversion happens at the *collection* level, not the per-element level.

## Python SDK design

### Current pain

- `instance.decision_variables_df()` and the ~9 other `*_df` methods on
  `Instance`, plus matching ones on `Solution` and `SampleSet`, return wide
  DataFrames where every row carries `name`, `subscripts`, `description`,
  `parameters.x`, `parameters.y`, …
- Metadata columns are duplicated across every stage's df. A constraint with
  rich metadata appears in `instance.regular_constraints_df()`,
  `solution.constraints_df()`, and `sample_set.constraints_df()` with the same
  metadata replicated each time.
- `parameters.{key}` is a wide-column with **data-dependent column names**.
  Aggregation, filtering, and union across instances are awkward.

### Target API

Three orthogonal axes:

1. **Core df** — type-specific columns and `id` only.
2. **Metadata df** — id-keyed sidecar with `name`, `subscripts`, `description`,
   `provenance`. One per collection kind.
3. **Long-format auxiliaries** — `parameters` (and `provenance`, optional)
   exposed as `(id, key, value)` long-format DataFrames.

```python
# Decision variables
df    = instance.decision_variables_df()       # id, kind, lower, upper, substituted_value
meta  = instance.variable_metadata_df()        # id, name, subscripts, description
prms  = instance.variable_parameters_df()      # id, key, value

# Constraints (4 kinds, 3 stages each)
df    = instance.regular_constraints_df()      # id, equality, function_type, used_ids
meta  = instance.constraint_metadata_df(kind="regular")
                                               # id, name, subscripts, description, provenance
prms  = instance.constraint_parameters_df(kind="regular")
                                               # id, key, value

# User joins as needed
df.merge(meta, on="id", how="left")
```

`removed_reason` stays on the core df as `removed_reason` (string) +
`removed_reason.{param}` if we keep it wide, or moves to a long
`removed_reasons_df(kind=...)` for consistency. Open question below.

### Why JOIN-based is the right shape here

- Metadata is **shared across stages** — the same metadata applies to a
  Created constraint, its Evaluated form in a Solution, and its Sampled form
  in a SampleSet. With wide dfs we copied it three times; with a sidecar
  metadata df, the user fetches it once.
- Metadata is **shared across active/removed** within a collection. Today
  `removed_constraints_df` and `constraints_df` carry separate metadata
  copies of the same id-keyed rows.
- `parameters` long-format frees the column space from data dependence — same
  schema regardless of which keys appear in which instance.
- The Rust SoA store maps directly to a per-column pandas/polars DataFrame.
  Bulk construction from `FnvHashMap<ID, …>` is one allocation per column.

### `ToPandasEntry` impact

`python/ommx/src/pandas.rs` currently has ~16 `ToPandasEntry` impls, each
producing a wide row dict. Under JOIN-based:

- **Core dfs** keep `ToPandasEntry`: row-based construction is fine for the
  small set of type-specific columns. We strip the `set_metadata(...)` and
  `set_parameter_columns(...)` calls from each impl.
- **Metadata dfs** are built column-wise from the SoA store, not via
  `ToPandasEntry`. New helper `metadata_store_to_dataframe(store)` walks the 4
  fields of the store and emits columns directly.
- **Parameters dfs** are built by flattening the SoA `FnvHashMap<ID,
  FnvHashMap<String, String>>` into three parallel vectors (`id`, `key`,
  `value`) and constructing a DataFrame.

Net result: roughly half the `ToPandasEntry` boilerplate disappears, and the
metadata path stops going through Python `dict` allocation per row.

### `subscripts` / `provenance` representation

- `subscripts`: kept as a List<Int64> column (one row per id). It is part of
  the variable / constraint identity and is consumed as a tuple, not
  aggregated cross-row. List-of-int columns are well-supported in pandas
  (object) and natively typed in polars.
- `provenance`: long format `(id, step, source_kind, source_id)` — one row per
  `(id, step)` pair. Provenance chains are queries (e.g. "which one-hot
  constraints became regular constraints?") and the long shape is the natural
  one for that.

## Migration strategy

Two stages, executed in order. Doing them in the opposite order forces an
ugly intermediate state where the Rust SoA store has to be re-marshalled into
wide-DataFrame Python output.

### Stage 1: Python API → JOIN-based (first)

- Strip metadata columns from all `*_df` methods on `Instance`, `Solution`,
  `SampleSet`.
- Add new `*_metadata_df`, `*_parameters_df`, `*_provenance_df` methods
  sourcing from the *current* AoS metadata field.
- Migration note in `PYTHON_SDK_MIGRATION_GUIDE.md`: any user code that read
  `df["name"]` or `df["parameters.{key}"]` must now `df.merge(metadata_df,
  on="id")` or pivot the long parameters df.
- Internal Rust types unchanged.

### Stage 2: Rust SoA migration (second)

- Add `ConstraintMetadataStore<T::ID>` and `VariableMetadataStore`.
- Remove `metadata` field from `DecisionVariable`, `Constraint<S>`,
  `IndicatorConstraint<S>`, `OneHotConstraint<S>`, `Sos1Constraint<S>`.
- Wire `Evaluate` / `From<v1::*>` paths to read/write metadata at the
  collection boundary.
- Python `*_metadata_df` / `*_parameters_df` switch to direct SoA → DataFrame
  bulk construction.

Both stages are v3-alpha breaking changes. Combined with
`SPECIAL_CONSTRAINTS_V3.md` they form the v3 metadata + special-constraints
package.

## Open questions

1. **Constraint kind dispatch in Python**: `constraint_metadata_df(kind="regular")`
   (one method, runtime kind) vs. `regular_constraint_metadata_df()` (four
   methods)? Engine-side it's the same code; the question is stub clarity vs.
   API surface size.
2. **`removed_reason` placement**: keep inline on each core df (current shape,
   wide `removed_reason.{key}`), or move to a separate
   `removed_reasons_df(kind=...)` long DataFrame?
3. **Builder-style metadata setter**: keep flat (`metadata_mut().set_name(id,
   ...)`) only, or also add `collection.insert_with(c, |m| m.name(...))` sugar
   for the common case?
4. **`parameters` storage on the Rust side**: `FnvHashMap<ID,
   FnvHashMap<String, String>>` (current shape) or transpose to
   `FnvHashMap<(ID, String), String>` to match the long-format Python API
   directly? The transposed form makes Python export trivial but penalizes
   the (rare) "all parameters of a single id" lookup.
5. **`subscripts` long format option**: in addition to the List<Int64> column,
   offer a `subscripts_df` with `(id, position, value)`? Useful when
   subscripts are heterogeneous lengths and the user wants to filter by
   `subscripts[0] == "i"`. Could land later if demand emerges.
6. **Polars as primary in Python**: out of scope here, but the JOIN-based API
   is polars-friendly. If we want to make polars primary in v3.x, we should
   ensure the Stage 1 API works equally well with `pl.DataFrame.join`.
