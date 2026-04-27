# Metadata Storage in OMMX v3 — Design Proposal

Status: **Draft / WIP**

This proposal is a **prerequisite** for `SPECIAL_CONSTRAINTS_V3.md` (PR #841).
The proto-schema redesign in #841 cannot be finalized without first deciding
how metadata (`name`, `subscripts`, `parameters`, `description`, `provenance`)
is stored at runtime and surfaced to users — the wire shape of
`ConstraintMetadata` (inline per message vs. top-level columnar map) only
makes sense once the runtime / Python-API direction is settled. So this
discussion was split out of #841 and runs first.

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

- **Blocks #841.** The proto v3 design in `SPECIAL_CONSTRAINTS_V3.md` extracts
  `ConstraintMetadata` as a shared sub-message but defers the inline-vs-
  top-level-columnar-map decision. That decision should follow, not lead,
  the runtime / Python-API direction set here.
- Metadata is on the hot path for memory (`logical_memory.rs` reports the
  per-row `Option`/`Vec`/`FnvHashMap` headers showing up under
  `Instance.constraint_collection;constraints;Constraint.metadata;…` and the
  same under `decision_variables`). Pulling it out lets the per-object struct
  shrink.
- The v3 alpha window is the right moment to break the wide-DataFrame Python
  API. Doing it after v3 GA would require another major.

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

Out of scope for this proposal. Once the runtime / Python-API direction here
is agreed, `SPECIAL_CONSTRAINTS_V3.md` (#841) finalizes the wire shape of
`ConstraintMetadata`. Two candidates:

- **Inline per constraint message** (currently sketched in #841). Wire format
  is AoS; the parse / serialize boundary translates to / from the SoA stores
  defined here.
- **Top-level `map<uint64, ConstraintMetadata>`** per collection. Wire format
  matches the runtime SoA store directly, eliminating the boundary translation
  but adding 4 extra top-level fields per stage on `Instance` / `Solution` /
  `SampleSet`.

Either is workable on top of the runtime design here. The decision belongs in
#841 and should be informed by what the parse / serialize boundary actually
looks like once this proposal is implemented (Stage 2 below).

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

pub struct ParametricInstance {
    // same treatment as Instance — also owns
    //   decision_variables: BTreeMap<VariableID, DecisionVariable>
    // directly, so it gets its own variable_metadata field.
    variable_metadata: VariableMetadataStore,   // new
    // (`parameters: BTreeMap<VariableID, v1::Parameter>` is unrelated metadata
    // and stays as-is.)
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

### Boundary changes

The SoA migration shifts several boundaries that are currently per-element:

- **Parsing** (`From<v1::Instance> for Instance`, etc., in
  `rust/ommx/src/instance/parse.rs` and `rust/ommx/src/constraint/parse.rs`).
  Today each element is parsed with its metadata; in Stage 2, parsing emits
  bare elements + a populated `*MetadataStore`. The natural locus is
  `From<v1::Instance>` and `From<Vec<v1::Constraint>> for ConstraintCollection<...>`.
- **Serialization** (`From<&Instance> for v1::Instance`,
  `From<(ID, EvaluatedConstraint)> for v1::EvaluatedConstraint`, etc.).
  Symmetric: serializers join element + metadata at the collection level.
- **`Evaluate` for `ConstraintCollection<T>`** already iterates the collection
  and constructs an `EvaluatedCollection<T>`; the metadata clone moves from
  per-constraint (currently `metadata: self.metadata.clone()` inside
  `SampledConstraintBehavior::get`) to a single store-level clone at the
  end.

### Other types affected

- **`#[derive(LogicalMemoryProfile)]`** is used on `DecisionVariable`,
  `ConstraintMetadata`, `DecisionVariableMetadata`, and the constraint
  structs. The new `*MetadataStore` types should also derive
  `LogicalMemoryProfile` so memory accounting under
  `Instance.constraint_collection;metadata;…` keeps working.
- **`pyo3-stub-gen`**: every new `*_df` method in Stage 1 and every renamed /
  removed method in Stages 1–2 needs the `gen_stub_pymethods` decorator and
  the corresponding `ommx.v1.__init__.py` regen via `task python:stubgen`.
  No new types are exposed to Python directly (the stores stay Rust-internal
  and surface only as DataFrames), so the stub surface change is limited to
  method signatures.

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
df       = instance.decision_variables_df()       # index=id; kind, lower, upper, substituted_value
meta     = instance.variable_metadata_df()        # index=id; name, subscripts, description
params   = instance.variable_parameters_df()      # columns: id, key, value (long format)

# Constraints (4 kinds, 3 stages each)
df       = instance.regular_constraints_df()      # index=id; equality, function_type, used_ids
meta     = instance.regular_constraint_metadata_df()
                                                  # index=id; name, subscripts, description, provenance
params   = instance.regular_constraint_parameters_df()
                                                  # columns: id, key, value (long format)

# User joins as needed (existing helpers like entries_to_dataframe set id as the
# index, so .join() composes naturally; .merge(on="id") works equally if the
# user resets the index first).
df.join(meta, how="left")
```

Removed reasons move to a separate long-format DataFrame for consistency:

```python
removed = instance.regular_constraint_removed_reasons_df()
                                                  # columns: id, reason, key, value
```

This matches what `Solution` already does today (`removed_reasons_df`,
`indicator_removed_reasons_df`) and aligns with `RemovedReason` being
collection-level metadata in Rust, not part of the constraint itself.

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

Three stages overall. This proposal covers Stage 1 and Stage 2; Stage 3
(proto) lives in #841 and is informed by the parse / serialize boundary that
emerges from Stage 2.

```
Stage 1 (this PR)        Stage 2 (this PR)         Stage 3 (#841)
┌──────────────────┐    ┌─────────────────────┐    ┌────────────────────┐
│ Python *_df API  │ →  │ Rust SoA metadata   │ →  │ proto wire shape   │
│ split into       │    │ stores;             │    │ for ConstraintMeta │
│ JOIN-based dfs.  │    │ remove metadata     │    │ decided based on   │
│ Internal Rust    │    │ from Constraint<S>  │    │ Stage 2 boundary.  │
│ AoS unchanged.   │    │ and DecisionVar.    │    │                    │
└──────────────────┘    └─────────────────────┘    └────────────────────┘
```

### Stage 1: Python API → JOIN-based

- Strip metadata columns from all `*_df` methods on `Instance`,
  `ParametricInstance`, `Solution`, `SampleSet`.
- Add new `*_metadata_df`, `*_parameters_df`, `*_provenance_df`,
  `*_removed_reasons_df` methods sourcing from the *current* AoS metadata
  field.
- Migration note in `PYTHON_SDK_MIGRATION_GUIDE.md`: any user code that read
  `df["name"]` or `df["parameters.{key}"]` must now `df.join(metadata_df)` or
  pivot the long parameters df.
- Internal Rust types unchanged. `ToPandasEntry` impls keep reading
  `c.metadata.*` directly — but the metadata-extracting helpers
  (`set_metadata`, `set_parameter_columns`) move out of the per-row impls and
  into bulk column builders that walk the AoS metadata field once per
  collection.

### Stage 2: Rust SoA migration

- Add `ConstraintMetadataStore<T::ID>` and `VariableMetadataStore` (see "Rust
  SDK design").
- Remove `metadata` field from `DecisionVariable`, `Constraint<S>`,
  `IndicatorConstraint<S>`, `OneHotConstraint<S>`, `Sos1Constraint<S>`.
- Move `From<v1::*>` parse / serialize boundaries from per-element to
  per-collection (see "Boundary changes" below).
- Stage 1's bulk column builders switch from "walk AoS field per element"
  to "read SoA store directly" — same DataFrame columns, no API churn.

### Stage 3: proto wire shape (deferred to #841)

Once Stage 2 lands, the parse / serialize boundary is concretely defined and
#841 can choose between the inline (per-message `ConstraintMetadata`) and
the top-level (`map<uint64, ConstraintMetadata>` per collection) wire shapes
based on what actually maps cleanly to the SoA stores.

### Why not Rust-first

A defensible alternative is to land Stage 2 first and let the SoA store come
out of `From<v1::*>` parse boundaries naturally — the per-collection
conversion shape Codex flagged is a clean cut. We don't, because:

- Stage 1 alone is a useful and shippable improvement (de-duplication,
  long-format parameters, no `parameters.{key}` data-dependent columns) even
  if Stage 2 slips.
- Stage 1 forces clarity on the `*_df` schema, which in turn determines what
  the SoA stores actually need to expose. Doing it second risks designing the
  Rust store and then realizing the Python API needs one more axis.
- Stage 1 surfaces the modeling-API question (next subsection) without
  dragging proto changes along.

### Standalone-constraint impact on Python modeling

Today users build constraints with chains like
`(x[0] + x[1] == 1).add_name("c").add_subscripts([0])` and then hand them to
`Instance.from_components`. Once Stage 2 strips metadata from
`Constraint<S>`, the Python wrapper for a standalone constraint can no
longer carry `name`/`subscripts` either.

Two options for the Python side:

1. **Staging wrapper.** The Python `Constraint` wrapper keeps a small inline
   metadata bag while it's standalone. `Instance.from_components` (and the
   constraint-collection `add_*` methods) drains the bag into the
   `*MetadataStore` at insertion time.
2. **Insertion-time API only.** Drop `add_name` / `add_subscripts` from
   standalone constraints entirely; require users to pass metadata as
   keyword args at insertion (`instance.add_constraint(c, name="c",
   subscripts=[0])`) or set it post-insertion via the metadata accessor.

(1) preserves the current chain ergonomics and is recommended unless we have
appetite for the breaking change to the modeling chain. (2) is cleaner but
breaks every notebook.

Stage 1 doesn't trigger this question (Rust still owns the metadata
inline); Stage 2 does. The Python wrapper change should land in the same PR
as Stage 2.

## Open questions

Each item lists a working recommendation; flagged for explicit sign-off
before implementation.

1. **Constraint kind dispatch in Python**: `constraint_metadata_df(kind="regular")`
   (one method, runtime kind) vs. `regular_constraint_metadata_df()` (four
   methods).
   - **Recommendation: four explicit methods.** `pyo3-stub-gen` produces
     concrete pymethod stubs per method, so explicit names give better
     IDE / type-checker discoverability than a runtime `kind` arg.
2. **`removed_reason` placement**: inline columns on the core df vs. a
   separate long-format `*_removed_reasons_df`.
   - **Recommendation: separate long-format df.** `Solution` already exposes
     `removed_reasons_df` / `indicator_removed_reasons_df` this way, and
     `RemovedReason` is collection-level metadata in Rust, not part of the
     constraint. Reflected in the "Target API" code block above.
3. **Builder-style metadata setter**: flat (`metadata_mut().set_name(id,
   ...)`) only, vs. also `collection.insert_with(c, |m| m.name(...))` sugar.
   - **Recommendation: add `insert_with`.** The flat form is too easy to
     forget right after insertion, especially when authoring constraints in
     a loop. Keep both.
4. **`parameters` storage on the Rust side**: `FnvHashMap<ID, FnvHashMap<String,
   String>>` vs. transposed `FnvHashMap<(ID, String), String>`.
   - **Recommendation: nested `FnvHashMap<ID, FnvHashMap<String, String>>`.**
     Matches the existing per-object metadata shape, makes "all parameters
     of one id" a natural lookup, and the long-format Python export is a
     one-pass flatten anyway.
5. **`subscripts` long format option**: in addition to the `List<Int64>`
   column, offer a `subscripts_df` with `(id, position, value)`.
   - **Recommendation: not in this proposal.** List column is enough for
     identity-style use. Add later if demand for positional filtering
     emerges.
6. **Polars as primary in Python**: out of scope here, but the JOIN-based
   API is polars-friendly. If we want to make polars primary in v3.x, we
   should ensure the Stage 1 API works equally well with `pl.DataFrame.join`.
   - **Recommendation: pandas stays primary for v3.** `PyDataFrame` is
     pandas-backed; polars promotion is a separate v3.x discussion.
7. **Stage 2 modeling-chain choice**: staging wrapper vs. insertion-time-only
   API for Python `Constraint`.
   - **Working assumption: staging wrapper** (option 1 above). Confirm before
     Stage 2 implementation lands.
