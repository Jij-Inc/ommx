# Metadata Storage in OMMX v3 — Design Proposal

Status: **Draft / WIP**

This proposal is a **prerequisite** for `SPECIAL_CONSTRAINTS_V3.md` (PR #841).
The proto-schema redesign in #841 cannot be finalized without first deciding
how metadata (`name`, `subscripts`, `parameters`, `description`, `provenance`)
is stored at runtime and surfaced to users — the wire shape of
`ConstraintMetadata` (inline per message vs. top-level columnar map) only
makes sense once the runtime / Python-API direction is settled. So this
discussion was split out of #841 and runs first.

This is a single connected redesign covering Rust SDK runtime layout and
Python SDK API surface. We don't break it into formal stages — every part
needs to land within the v3 alpha window for the API to read coherently.

## Goal

Today the same constraint / decision-variable fact lives in **three places**:

1. Rust — `BTreeMap<ID, T>`, with `metadata: TMetadata` inlined into each `T`.
2. Python — `instance.constraints: dict[id, Constraint object]`, where
   `Constraint.name`, `Constraint.subscripts`, … are getters that copy out
   what was inlined in (1).
3. Python — `instance.constraints_df()`, a wide DataFrame with the same
   metadata replicated as columns next to the type-specific data.

This metadata is rarely consulted by internal logic — `evaluate`, `parse`,
`substitute`, etc. never read it — but it is copied around on every clone,
serialized on every write, exposed via per-object getters, and duplicated
into every wide `*_df` API. The duplication shows up in memory accounting
(`logical_memory.rs` reports per-row `Option`/`Vec`/`FnvHashMap` headers
under `Instance.constraint_collection;constraints;Constraint.metadata;…`)
and in API surface drift (when a new metadata field is added it has to be
wired through the struct, the getter, and the DataFrame builder).

We want a single source of truth on the Rust side and one well-defined
"shape" for each kind of access on the Python side.

## Why now

- **Blocks #841.** The proto v3 design in `SPECIAL_CONSTRAINTS_V3.md`
  extracts `ConstraintMetadata` as a shared sub-message but defers the
  inline-vs-top-level-columnar-map decision. That decision should follow,
  not lead, the runtime / Python-API direction set here.
- The v3 alpha window is the right moment to break the wide-DataFrame
  Python API and the per-object metadata getters. Doing it after v3 GA
  would require another major.

## Target shape (one picture)

### Rust

- Metadata moves into ID-keyed Struct-of-Arrays stores. The store sits at
  the collection layer:
  - `ConstraintCollection<T>` owns `ConstraintMetadataStore<T::ID>`.
  - `Instance` and `ParametricInstance` own `VariableMetadataStore` directly
    (no `DecisionVariableCollection` for symmetry's sake).
  - `EvaluatedCollection<T>` and `SampledCollection<T>` carry the same
    `ConstraintMetadataStore<T::ID>` so Solution / SampleSet share one
    metadata source per collection.
- `DecisionVariable`, `Constraint<S>`, `IndicatorConstraint<S>`,
  `OneHotConstraint<S>`, `Sos1Constraint<S>` lose their `metadata` field.
  Per-object structs shrink to the type's intrinsic data only.
- Parse / serialize boundaries move from per-element to per-collection so
  metadata can be read / written column-by-column.

### Python

- `instance.constraints`, `decision_variables`, `*_constraints` become
  `pandas.Series[ID -> Object]` — index = ID, value = the (now slimmer)
  PyO3 wrapper object. Series indexing (`s.loc[id]`, `s.iloc[i]`,
  iteration, boolean indexing) replaces the current dict / list APIs.
- `*_df` methods are explicitly **derived views**: each one is the
  Series joined with the relevant metadata / parameters / removed-reason
  sidecar dfs.
- Sidecar DataFrames (`*_metadata_df`, `*_parameters_df` long format,
  `*_provenance_df` long format, `*_removed_reasons_df` long format) are
  bulk-built from the Rust SoA store, one column allocation per field.
- Wrapper objects (`Constraint`, `DecisionVariable`, …) carry only the
  type's intrinsic data — `equality`, `function`, `kind`, `bound`, etc.
  All `.name` / `.subscripts` / `.parameters` / `.description` getters
  are removed; the only path to metadata is the metadata df.

### Proto

Out of scope here. Once this lands and the parse / serialize boundary is
concrete, #841 picks the wire shape (`ConstraintMetadata` inline per
message vs. top-level `map<uint64, ConstraintMetadata>` per collection).
Either is workable on top of the Rust SoA stores.

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

`provenance` lives only on constraints; the variable store omits it.
Otherwise the two stores are structurally identical.

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
    decision_variables:              BTreeMap<VariableID, DecisionVariable>,
    constraint_collection:           ConstraintCollection<Constraint>,
    indicator_constraint_collection: ConstraintCollection<IndicatorConstraint>,
    one_hot_constraint_collection:   ConstraintCollection<OneHotConstraint>,
    sos1_constraint_collection:      ConstraintCollection<Sos1Constraint>,

    variable_metadata: VariableMetadataStore,   // new
    // existing fields …
}

pub struct ParametricInstance {
    // same treatment — ParametricInstance also owns
    //   decision_variables: BTreeMap<VariableID, DecisionVariable>
    // directly.
    variable_metadata: VariableMetadataStore,   // new
    // (`parameters: BTreeMap<VariableID, v1::Parameter>` is unrelated metadata
    // and stays as-is.)
}
```

Why these levels:

- For constraints, `ConstraintCollection<T>` already owns active/removed
  split and is generic over constraint type — putting the store there keeps
  the `relax` / `restore` pair touch-free (active ↔ removed transitions
  don't move metadata at all). The same store rides through to
  `EvaluatedCollection<T>` / `SampledCollection<T>` on the Solution /
  SampleSet side.
- For variables, there is no analogous `DecisionVariableCollection` and
  adding one only to host metadata would be over-engineering — `Instance`
  and `ParametricInstance` already own `BTreeMap<VariableID,
  DecisionVariable>` directly. We just add a sibling field.

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

Standalone constraints (`Constraint::equal_to_zero(f)`,
`OneHotConstraint::new(...)`, etc.) carry no metadata. Metadata is set
after inserting into a collection:

```rust
let id = collection.insert(Constraint::equal_to_zero(f));
collection.metadata_mut().set_name(id, "demand_balance");
collection.metadata_mut().push_subscripts(id, [i, j]);
```

A builder-style helper is provided for the common "insert with metadata"
case:

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

`ConstraintMetadataView::parameters()` returns `&EMPTY_MAP` for absent
keys (static const) so the `Option<FnvHashMap<…>>` storage doesn't leak
through the public API.

### Parse / serialize boundaries

The boundaries currently work per-element and need to move to per-collection:

- **Parsing** (`From<v1::Instance> for Instance`, etc., in
  `rust/ommx/src/instance/parse.rs` and
  `rust/ommx/src/constraint/parse.rs`). Today each element is parsed with
  its metadata; after the refactor, parsing emits bare elements and a
  populated `*MetadataStore`. The natural locus is `From<v1::Instance>`
  and `From<Vec<v1::Constraint>> for ConstraintCollection<...>`.
- **Serialization** (`From<&Instance> for v1::Instance`,
  `From<(ID, EvaluatedConstraint)> for v1::EvaluatedConstraint`, etc.).
  Symmetric: serializers join element + metadata at the collection level.
- **`Evaluate` for `ConstraintCollection<T>`** already iterates the
  collection and constructs an `EvaluatedCollection<T>`. The metadata
  clone moves from per-constraint (currently `metadata:
  self.metadata.clone()` inside `SampledConstraintBehavior::get`) to a
  single store-level clone at the end.

```rust
impl<T: ConstraintType> EvaluatedCollection<T> {
    pub fn into_v1(self) -> Vec<v1::EvaluatedConstraint> {
        let (constraints, _removed) = self.into_parts();
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

### Other types affected

- **`#[derive(LogicalMemoryProfile)]`** is currently on `DecisionVariable`,
  `ConstraintMetadata`, `DecisionVariableMetadata`, and the constraint
  structs. The new `*MetadataStore` types should derive
  `LogicalMemoryProfile` so memory accounting under
  `Instance.constraint_collection;metadata;…` keeps working.
- **`pyo3-stub-gen`**: every renamed / removed / added Python method below
  needs the `gen_stub_pymethods` decorator and the corresponding
  `ommx.v1.__init__.py` regen via `task python:stubgen`. The stores are
  not exposed to Python directly (they surface only as Series and
  DataFrames), so the stub surface change is limited to method
  signatures.

## Python SDK design

### Single source of truth

```
            Rust SoA store (BTreeMap<ID, T> + ConstraintMetadataStore<ID>)
                         │
                         ▼
        instance.constraints  ── pandas.Series[ID -> Constraint object]
                         │
                ┌────────┴────────────────────┐
                │                             │
       sidecar metadata df             core columns extracted
       sidecar parameters df          from the Constraint object
       sidecar removed_reasons df             │
                └─────────────┬───────────────┘
                              ▼
                    instance.constraints_df()
                    (joined view, never canonical)
```

Wrapper objects carry only the type's intrinsic data. They have no
`.name` / `.subscripts` / `.parameters` / `.description`. The sole path
to metadata is the sidecar df keyed by id.

### Collection accessors → Series

```python
s = instance.constraints                  # pandas.Series[ConstraintID -> Constraint]
s.loc[5]                                  # individual Constraint object
s.loc[5].equality                         # OK — type-intrinsic getter
s.loc[5].name                             # AttributeError — removed

list(s.index)                             # all constraint IDs
for cid, c in s.items(): ...              # iteration

# decision variables and the other constraint kinds get the same treatment
instance.decision_variables               # Series[VariableID -> DecisionVariable]
instance.indicator_constraints            # Series[IndicatorConstraintID -> IndicatorConstraint]
instance.one_hot_constraints              # Series[OneHotConstraintID -> OneHotConstraint]
instance.sos1_constraints                 # Series[Sos1ConstraintID -> Sos1Constraint]

solution.constraints                      # Series[ConstraintID -> EvaluatedConstraint]
sample_set.constraints                    # Series[ConstraintID -> SampledConstraint]
```

The Series carries Python wrapper objects (object dtype). Per-element
efficiency is the same as the old `dict[ID, Constraint]` — this is an
API integration win, not a perf change. Modeling code that built
constraints standalone and inserted them into a dict-like collection
keeps working with the natural `instance.add_constraint(c)` /
`s.add(c)` flow; index-based pandas operations (`.loc`, `.iloc`, boolean
indexing, `.items()`) are now available out of the box.

### `*_df` methods → derived views

Each `*_df` is explicitly a derived view: type-specific core columns
extracted from the Series, joined with the relevant sidecar dfs.

```python
# Decision variables
df       = instance.decision_variables_df()       # index=id; kind, lower, upper, substituted_value
meta     = instance.variable_metadata_df()        # index=id; name, subscripts, description
params   = instance.variable_parameters_df()      # columns: id, key, value (long format)

# Constraints (per kind)
df       = instance.regular_constraints_df()      # index=id; equality, function_type, used_ids
meta     = instance.regular_constraint_metadata_df()
                                                  # index=id; name, subscripts, description
provs    = instance.regular_constraint_provenance_df()
                                                  # columns: id, step, source_kind, source_id (long)
params   = instance.regular_constraint_parameters_df()
                                                  # columns: id, key, value (long format)
removed  = instance.regular_constraint_removed_reasons_df()
                                                  # columns: id, reason, key, value (long format)

# Joining is explicit
df.join(meta, how="left")
```

`*_df` is what users call when they want a single rectangular table for
analysis; the Series is what users call when they want individual wrapper
objects. Both are derived from the same Rust SoA — neither is canonical
relative to the other.

### `ToPandasEntry` restructuring

`python/ommx/src/pandas.rs` currently has ~16 `ToPandasEntry` impls,
each producing a wide row dict.

- **Core dfs** keep `ToPandasEntry`: row-based construction is fine for
  the small set of type-specific columns. We strip the
  `set_metadata(...)` and `set_parameter_columns(...)` calls from each
  impl.
- **Metadata dfs** are built column-wise from the SoA store, not via
  `ToPandasEntry`. New helper `metadata_store_to_dataframe(store)` walks
  the fields of the store and emits columns directly.
- **Long-format dfs** (`parameters`, `provenance`, `removed_reasons`)
  are built by flattening the SoA `FnvHashMap` into parallel vectors
  and constructing a DataFrame.
- **Series accessors** wrap each Rust SoA's `BTreeMap<ID, T>` into a
  `pandas.Series[ID -> Object]` — one Python list of wrappers + one
  Index of IDs, no per-row dict allocation.

Net result: roughly half the `ToPandasEntry` boilerplate disappears,
metadata stops going through Python `dict` allocation per row, and the
Series path is allocation-symmetric to today's `dict` path.

### Standalone constraints and the modeling chain

Today users write
`(x[0] + x[1] == 1).add_name("c").add_subscripts([0])` and hand the
result to `Instance.from_components`. With metadata removed from the
Rust `Constraint<S>` and from the Python wrapper's getters, the chain
needs an explicit landing place.

**Working assumption**: keep `add_name` / `add_subscripts` /
`add_description` on the Python `Constraint` wrapper, but make them
populate a small inline staging bag rather than the (now-absent) Rust
`metadata` field. `Instance.from_components` and the
`add_constraint(...)` family drain the bag into the relevant
`*MetadataStore` at insertion time.

The alternative (drop the chain entirely; require keyword args at
insertion or post-insertion mutation via the metadata accessor) is
cleaner but breaks every modeling notebook. Confirm before
implementation.

### `subscripts` / `provenance` representation

- `subscripts`: `List<Int64>` column on the metadata df (one row per id).
  Part of the variable / constraint identity, consumed as a tuple, not
  aggregated cross-row. List-of-int columns work in pandas (object
  dtype) and are natively typed in polars.
- `provenance`: long format `(id, step, source_kind, source_id)` — one
  row per `(id, step)` pair. Provenance chains are queries (e.g. "which
  one-hot constraints became regular constraints?") and the long shape
  is the natural one for that.

## Breaking changes

User-visible breakage relative to v3 alpha 2:

- `instance.constraints`, `decision_variables`, `*_constraints` change
  type from `dict` / `list` to `pandas.Series`. Most existing usage
  (`s[id]`, iteration, `.items()`) keeps working because Series
  reproduces those; positional reliance on `list[...]` ordering for
  decision variables breaks.
- `Constraint.name`, `.subscripts`, `.parameters`, `.description` (and
  the same on `DecisionVariable` and the special-constraint wrappers)
  are removed. Users must read from the metadata df.
- `*_df` methods no longer carry `name`, `subscripts`, `description`,
  or `parameters.{key}` columns. Users must `df.join(meta_df)` or pivot
  the long parameters df.
- New `*_metadata_df`, `*_parameters_df`, `*_provenance_df`,
  `*_removed_reasons_df` methods are added per collection kind.
- The Rust `metadata` field on `DecisionVariable` and `Constraint<S>`
  is removed. Downstream Rust crates touching `c.metadata.*` directly
  must switch to the collection's `metadata()` accessor.

A new section in `PYTHON_SDK_MIGRATION_GUIDE.md` covers the Python side
in detail.

## Open questions

Each item lists a working recommendation; flagged for explicit sign-off
before implementation.

1. **Constraint kind dispatch in Python**:
   `constraint_metadata_df(kind="regular")` (one method, runtime kind)
   vs. `regular_constraint_metadata_df()` (four methods).
   - **Recommendation: four explicit methods.** `pyo3-stub-gen`
     produces concrete pymethod stubs per method, so explicit names
     give better IDE / type-checker discoverability than a runtime
     `kind` arg.
2. **`removed_reason` placement**: inline columns on the core df vs. a
   separate long-format `*_removed_reasons_df`.
   - **Recommendation: separate long-format df.** `Solution` already
     exposes `removed_reasons_df` / `indicator_removed_reasons_df`
     this way, and `RemovedReason` is collection-level metadata in
     Rust, not part of the constraint.
3. **Builder-style metadata setter**: flat
   (`metadata_mut().set_name(id, ...)`) only, vs. also
   `collection.insert_with(c, |m| m.name(...))` sugar.
   - **Recommendation: add `insert_with`.** The flat form is too easy
     to forget right after insertion, especially when authoring
     constraints in a loop. Keep both.
4. **`parameters` storage on the Rust side**: `FnvHashMap<ID,
   FnvHashMap<String, String>>` vs. transposed `FnvHashMap<(ID,
   String), String>`.
   - **Recommendation: nested `FnvHashMap<ID, FnvHashMap<String,
     String>>`.** Matches the existing per-object metadata shape,
     makes "all parameters of one id" a natural lookup, and the
     long-format Python export is a one-pass flatten anyway.
5. **`subscripts` long format option**: in addition to the
   `List<Int64>` column, offer a `subscripts_df` with `(id, position,
   value)`.
   - **Recommendation: not in this proposal.** List column is enough
     for identity-style use. Add later if demand for positional
     filtering emerges.
6. **Polars as primary in Python**: out of scope here, but the
   JOIN-based API is polars-friendly.
   - **Recommendation: pandas stays primary for v3.** `PyDataFrame`
     is pandas-backed; polars promotion is a separate v3.x discussion.
7. **Modeling-chain choice**: staging wrapper (recommended) vs.
   insertion-time-only API for Python `Constraint`.
   - **Working assumption: staging wrapper.** Confirm before
     implementation.
8. **Series wrapper-object dtype**: object-dtype Series with PyO3
   wrapper values is structurally fine but invites
   `s.apply(lambda c: c.equality)` patterns that could be served more
   efficiently by the corresponding df column.
   - **Recommendation: document Series as the dict/list replacement
     and `*_df` as the bulk-analysis surface.** No type-level
     enforcement; trust users to pick the right tool.
