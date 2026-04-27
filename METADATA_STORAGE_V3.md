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
Python SDK API surface. The document describes the target shape; phasing of
the implementation across PRs is decided in the implementation issues, not
here. (Recommended: split implementation into Rust-side SoA + parse boundary,
Python-side Series + sidecar dfs + back-references, and the doc /
migration-guide updates as separate PRs even though the design doc treats
them as one piece.)

## Goal

Today the same fact lives in three places that have to stay in sync by
hand:

1. Rust — `BTreeMap<ID, T>`, with `metadata: TMetadata` inlined into each `T`.
2. Python — `instance.constraints: dict[id, Constraint object]`, with
   getters `Constraint.name`, `Constraint.subscripts`, … that copy out
   what was inlined in (1).
3. Python — `instance.constraints_df()`, a wide DataFrame with the same
   metadata replicated as columns next to the type-specific data.

We want one **canonical storage** in Rust and well-defined **derived views**
on top of it. The user-visible surfaces (the PyO3 wrapper objects, the new
`Series` collection accessors, the `*_df` methods) all stay — but they
all read from the same SoA store rather than each carrying its own copy.

The duplication shows up in memory accounting (`logical_memory.rs`
reports per-row `Option`/`Vec`/`FnvHashMap` headers under
`Instance.constraint_collection;constraints;Constraint.metadata;…`) and
in API surface drift (when a new metadata field is added it has to be
wired through the struct, the getter, and the DataFrame builder).

Internal Rust logic is mostly metadata-blind, but not entirely: parsing
and evaluation skip metadata, while `rust/ommx/src/sample_set/extract.rs`
filters and dispatches on `metadata.name`, `metadata.subscripts`, and
`metadata.parameters`. Those call sites move to reading the collection's
SoA store; the behavior they implement is unchanged.

## Why now

- **Blocks #841.** The proto v3 design in `SPECIAL_CONSTRAINTS_V3.md`
  extracts `ConstraintMetadata` as a shared sub-message but defers the
  inline-vs-top-level-columnar-map decision. That decision should follow,
  not lead, the runtime / Python-API direction set here.
- The v3 alpha window is the right moment to break the Python `dict` /
  wide-DataFrame API. Doing it after v3 GA would require another major.

## Target shape (one picture)

### Rust

- Metadata moves into ID-keyed Struct-of-Arrays stores. The store sits at
  the collection layer:
  - `ConstraintCollection<T>` owns `ConstraintMetadataStore<T::ID>`.
  - `Instance` and `ParametricInstance` own `VariableMetadataStore`
    directly (no `DecisionVariableCollection` for symmetry's sake).
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
  `pandas.Series[ID -> Object]` — index = ID, value = the PyO3 wrapper
  object. Series indexing replaces dict / list APIs.
- `*_df` methods are explicitly **derived views**: each one returns
  type-specific core columns extracted from the SoA store, with no
  metadata columns inlined. They do **not** join sidecars themselves;
  callers either join the sidecar dfs explicitly or use the
  `*_full_df` helper covered below.
- Sidecar DataFrames are bulk-built from the Rust SoA store with one
  column allocation per field. Constraint-side methods take a
  `kind: Literal["regular","indicator","one_hot","sos1"]` argument
  (`constraint_metadata_df(kind=...)`,
  `constraint_parameters_df(kind=...)`,
  `constraint_provenance_df(kind=...)`,
  `constraint_removed_reasons_df(kind=...)`); decision-variable-side
  methods need no kind (`variable_metadata_df`,
  `variable_parameters_df`).
- **Wrapper objects keep their metadata getters** (`Constraint.name`,
  `.subscripts`, `.parameters`, `.description`; same on the other
  wrappers). For wrappers obtained from a collection ("attached"), the
  getters read the collection's SoA store via a back-reference. For
  wrappers built standalone in a modeling chain, the getters read a
  staging bag that drains into the SoA store on insertion. The user
  sees the same surface either way; the wrapper just doesn't own the
  metadata bytes anymore.

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

- For constraints, `ConstraintCollection<T>` already owns the active /
  removed split and is generic over constraint type — putting the store
  there keeps the `relax` / `restore` pair touch-free (active ↔ removed
  transitions don't move metadata at all). The same store rides through
  to `EvaluatedCollection<T>` / `SampledCollection<T>` on the Solution /
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
`OneHotConstraint::new(...)`, etc.) carry no metadata at the Rust
level. Insertion drains a staging bag (Python wrappers) or accepts an
explicit metadata argument:

```rust
let id = collection.insert(Constraint::equal_to_zero(f));
collection.metadata_mut().set_name(id, "demand_balance");
collection.metadata_mut().push_subscripts(id, [i, j]);

// or via the convenience builder. Builder methods correspond to the
// existing Python `set_*` semantics (replace, not extend) since the
// builder always starts from an empty store:
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

The internal call sites that used to read `c.metadata.*` directly (e.g.
`rust/ommx/src/sample_set/extract.rs`'s `metadata.name`,
`metadata.subscripts`, `metadata.parameters` filters) switch to
`collection.metadata().name(id)` / `.subscripts(id)` / `.parameters(id)`
accessors. Behavior unchanged.

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

### Other types affected

- **`#[derive(LogicalMemoryProfile)]`** is currently on `DecisionVariable`,
  `ConstraintMetadata`, `DecisionVariableMetadata`, and the constraint
  structs. The new `*MetadataStore` types should derive
  `LogicalMemoryProfile` so memory accounting under
  `Instance.constraint_collection;metadata;…` keeps working.
- **`pyo3-stub-gen`**: every renamed / removed / added Python method
  below needs the `gen_stub_pymethods` decorator and the corresponding
  `ommx.v1.__init__.py` regen via `task python:stubgen`. The stores are
  not exposed to Python directly; they surface via wrapper getters,
  Series accessors, and DataFrames.

## Python SDK design

### Layered views over the Rust SoA store

```
                  Rust SoA store (canonical)
                  ┌────────────────────────────┐
                  │ ConstraintCollection<T>    │
                  │   active / removed         │
                  │   metadata: SoA store      │
                  └────────────────────────────┘
                              │
                              ▼ (PyO3 boundary)
              ┌───────────────────────────────────┐
              │ ConstraintCollection (Py wrapper) │
              └─────┬─────────────────────────────┘
                    │
       ┌────────────┼─────────────────────┐
       ▼            ▼                     ▼
  Series         Constraint object     constraints_df / constraint_metadata_df /
  (per-id        (with .name /         constraint_parameters_df / etc.
   wrapper       .subscripts / …       (bulk-built from the SoA store via
   handles)      back-referenced       column-wise builders; kind dispatched
                 to the store)         via Literal + @overload)
```

Wrapper objects, Series, and DataFrames are three views over the same
store. The wrapper getters and the DataFrame columns produce the same
values for the same ID; the difference is bulk vs. per-id ergonomics.

### Wrapper objects with back-reference

PyO3 wrappers stay rich. The implementation is two-mode:

```rust
#[pyclass]
pub struct Constraint {
    inner: ConstraintInner,
}

enum ConstraintInner {
    /// Standalone — built via Constraint::equal_to_zero(f) or operator
    /// overloading. Holds owned core data and a metadata staging bag.
    /// Setters write to the bag; getters read it.
    Standalone {
        constraint: ommx::Constraint,
        staging:    ConstraintMetadataStaging,
    },
    /// Attached — obtained from a collection. Holds a back-reference to
    /// the parent Instance plus the constraint's id. Getters look up
    /// core data from the collection's BTreeMap and metadata from the
    /// SoA store. Setters write through to the SoA store.
    Attached {
        instance: Py<Instance>,
        kind:     ConstraintKind,
        id:       ConstraintID,
    },
}
```

`Instance.add_constraint(c)` (and the special-constraint equivalents)
takes a Standalone wrapper, drains its staging bag into the SoA store,
and returns a fresh Attached wrapper bound to that `id`. The original
Standalone wrapper is **transitioned in place**: its `inner` flips to
`Attached { instance, kind, id }` so subsequent `c.name`,
`c.add_name(...)`, etc. read / write the SoA store. Calling
`add_constraint(c)` a second time on an already-Attached wrapper
raises `ValueError("constraint already inserted")` rather than
silently re-inserting. Series-derived wrappers (`s.loc[id]`) are
also Attached, sharing the `Py<Instance>` of the parent.

Two Attached wrappers that point at the same id observe the same
state: a write through one (`a.name = "x"`) is visible through any
other (`b.name == "x"`) and through the metadata df on the next
call. Concurrency-wise this is the standard PyO3 borrow rule — the
SoA store is mutated through `Bound<'py, Instance>` borrowing, which
the runtime checks at `&mut` boundaries.

```python
# Standalone modeling — staging bag in the wrapper
c = (x[0] + x[1] == 1).add_name("balance").add_subscripts([0])
print(c.name)        # "balance" — read from staging bag

# Insertion — staging bag drains into the SoA store
attached = instance.add_constraint(c)
print(attached.name) # "balance" — read from SoA via back-reference

# Series access — Attached wrappers
s = instance.constraints
print(s.loc[5].name) # back-reference lookup; same value as the metadata df

# Mutation — write-through to SoA
attached.name = "demand_balance"
# or attached.add_name(...) keeping the chain shape
```

### Staleness / lifetime

Attached wrappers hold `Py<Instance>` (a refcounted handle). The
`Instance` stays alive as long as any wrapper points at it, so the back
reference can't dangle. Open semantic question:

- **`relax(id)`** moves the constraint to the removed map; the wrapper
  remains valid (the SoA metadata store is keyed by id regardless of
  active / removed).
- **`drop_constraint(id)`** (does not exist today; would be added if
  ever needed) would invalidate Attached wrappers for that id. Until
  it exists, this question is moot.

The simple rule: a wrapper's `id` stays in either the active or removed
map for the lifetime of the parent `Instance`, so getters never panic.

### Series-based collection accessors

```python
s = instance.constraints                  # pandas.Series[ConstraintID -> Constraint]
s.loc[5]                                  # individual Constraint object (Attached)
s.loc[5].equality                         # type-intrinsic getter
s.loc[5].name                             # metadata getter via back-reference

list(s.index)                             # all constraint IDs
for cid, c in s.items(): ...              # iteration

# decision variables and the other constraint kinds get the same treatment
instance.decision_variables               # Series[VariableID -> DecisionVariable]
instance.indicator_constraints            # Series[IndicatorConstraintID -> IndicatorConstraint]
instance.one_hot_constraints              # Series[OneHotConstraintID -> OneHotConstraint]
instance.sos1_constraints                 # Series[Sos1ConstraintID -> Sos1Constraint]

solution.constraints                      # Series[ConstraintID -> EvaluatedConstraint]
sample_set.constraints                    # Series[ConstraintID -> SampledConstraint]

# ParametricInstance follows the same surface as Instance: Series
# accessors for variables / constraints / special constraints, the
# corresponding *_df / *_metadata_df / *_full_df family, and back-
# referenced wrappers for individual elements. The (unrelated)
# parametric `parameters` field stays on its own dedicated accessor.
parametric_instance.constraints           # Series[ConstraintID -> Constraint]
parametric_instance.decision_variables    # Series[VariableID -> DecisionVariable]
```

The Series carries Attached wrapper objects (object dtype). Per-element
efficiency is the same as the old `dict[ID, Constraint]`. Indexing
operations users get for free vs. dict: `.loc`, `.iloc`, boolean
indexing, `.items()`, `.index`. Operations users lose vs. dict:

- **`s.values()` is NOT a method**; pandas `Series.values` is a
  property returning a numpy array. Existing `.values()` calls break
  loudly. Migration: `s.tolist()`, `list(s)`, or `for c in s:`.
- **`s[id]` works for an integer id** because Series allows index-by-
  label lookup with `[]`, but `.loc[id]` is the explicit form.
  Documentation should prefer `.loc[id]` to avoid the
  position-vs-label ambiguity.
- **`s.apply(lambda c: c.equality)` is an attractive nuisance**: it
  iterates Python-side and rebuilds the equality column row-by-row.
  The right answer is `instance.constraints_df()["equality"]`, which
  is bulk-built from the SoA. Document this; do not enforce.

### `*_df` methods → derived views

Each `*_df` is a derived view. Implementation reads the SoA store
directly via column-wise builders (no per-row dict construction).

```python
# Decision variables
df       = instance.decision_variables_df()       # index name=variable_id;
                                                  # kind, lower, upper, substituted_value
meta     = instance.variable_metadata_df()        # index name=variable_id;
                                                  # name, subscripts, description
params   = instance.variable_parameters_df()      # columns: variable_id, key, value (long format)

# Constraints — kind dispatched via Literal + @overload so the IDE / type
# checker still sees the kind-specific column schema for each call.
df       = instance.constraints_df(kind="regular")
                                                  # index name=regular_constraint_id;
                                                  # equality, function_type, used_ids
meta     = instance.constraint_metadata_df(kind="regular")
                                                  # index name=regular_constraint_id;
                                                  # name, subscripts, description
provenance_df = instance.constraint_provenance_df(kind="regular")
                                                  # columns: regular_constraint_id, step,
                                                  # source_kind, source_id (long)
params   = instance.constraint_parameters_df(kind="regular")
                                                  # columns: regular_constraint_id, key,
                                                  # value (long format)
removed  = instance.constraint_removed_reasons_df(kind="regular")
                                                  # columns: regular_constraint_id, reason,
                                                  # key, value (long format)

# Joining is explicit
df.join(meta, how="left")
```

`Solution` and `SampleSet` expose the same `*_df` family with stage-
appropriate column schemas. The method names match `Instance`:

```python
# Solution — evaluated stage
df = solution.constraints_df(kind="regular")
                                            # index name=regular_constraint_id;
                                            # equality, evaluated_value, feasible,
                                            # used_ids, dual_variable
df = solution.decision_variables_df()       # index name=variable_id;
                                            # kind, lower, upper, value
solution.constraint_metadata_df(kind=...)   # same metadata schema as Instance
solution.constraint_full_df(kind=...)       # core + metadata join
# … same family for solution.variable_*_df / solution.constraint_*_df

# SampleSet — sampled stage; columns are per-sample (value.{sid},
# feasible.{sid}, …) where sid is each SampleID.
df = sample_set.constraints_df(kind="regular")
df = sample_set.decision_variables_df()
sample_set.constraint_metadata_df(kind=...)
sample_set.constraint_full_df(kind=...)
```

The metadata / parameters / provenance / removed-reasons sidecar dfs
are stage-independent (they describe the same constraint), so on
`Solution` and `SampleSet` they return the same data they would on
the source `Instance` — there's no per-stage divergence to manage.

`*_df` (core only) is what users call when they want a single
rectangular table for analysis; `*_full_df` is the convenience
join (see "Avoiding cross-ID-space joins"); the Series is what users
call when they want individual wrapper objects; the wrapper getters
are what users call when they already hold one wrapper. Four
surfaces, one canonical store.

### Avoiding cross-ID-space joins

Each constraint kind has its own ID space (regular ID 5 ≠ indicator ID 5),
and decision variable IDs live in yet another space. With every df sharing
an `int64` index, `df.join()` between mismatched-kind dfs would silently
produce an incorrect-but-shaped result. We ward off that mistake at the
naming and helper layers:

1. **Distinct index names per ID space.** All dfs returned by the API set
   their index name to a kind-qualified label:

   ```
   variable_id                   # decision variables
   regular_constraint_id         # regular constraints
   indicator_constraint_id       # indicator constraints
   one_hot_constraint_id         # one-hot constraints
   sos1_constraint_id            # SOS1 constraints
   ```

   pandas `df.join(other)` aligns by index but does *not* enforce that
   the index names match. The qualified names alone won't stop a wrong
   join — but they're visible in `df.head()`, `df.info()`, IDE
   inspection, and migration-guide examples, so users see the mismatch
   immediately rather than chasing a silent bug downstream.

2. **One-step joined helpers per kind.** For the common "I want core
   columns plus name/subscripts/description in one frame" case, expose:

   ```python
   instance.constraints_full_df(
       kind="regular",
       include=("metadata",),     # default — name / subscripts / description
                                  # joined as columns on the core df
   )
   instance.constraints_full_df(
       kind="regular",
       include=("metadata", "parameters", "provenance",
                "removed_reasons"),
   )
   instance.decision_variables_full_df(
       include=("metadata", "parameters"),
   )
   ```

   `include` is a tuple of literal strings (typed via `Literal[...]`)
   selecting which sidecars to fold in. `"metadata"` is left-joined as
   columns; `"parameters"` / `"provenance"` / `"removed_reasons"` are
   left-joined after pivoting their long-format keys back to wide
   columns under namespaced prefixes (`parameters.{key}`,
   `provenance.{step}.source_id`, `removed_reasons.{key}`) — the
   resulting df is heterogeneous-column-shaped, so this form is
   strictly the convenience surface. Users who want clean long-form
   data for analysis still go through the primary `*_df` family.

   Users who only want a quick analysis frame call `*_full_df` and
   never write a manual `.join()` at all.

3. **Wrapper-object access stays the safest path for single-id
   lookups.** `s.loc[id].name` reads metadata via the back-reference
   without any join, so any code that operates a constraint at a time
   sidesteps the issue entirely. `*_df` joins are reserved for bulk
   analysis, where (1) and (2) cover the realistic mistake modes.

A stronger guarantee — encoding kind in the index dtype itself
(MultiIndex `(kind, id)`, or a custom ExtensionArray) — was considered
and rejected. MultiIndex is intrusive on every analysis call; a custom
ExtensionArray is a meaningful pandas integration and a maintenance
burden disproportionate to the failure mode it prevents. The qualified
index name + `*_full_df` helpers are sufficient.

### `ToPandasEntry` restructuring

`python/ommx/src/pandas.rs` currently has ~16 `ToPandasEntry` impls,
each producing a wide row dict.

- **Core dfs** keep `ToPandasEntry`: row-based construction is fine for
  the small set of type-specific columns. We strip the
  `set_metadata(...)` and `set_parameter_columns(...)` calls from each
  impl.
- **Metadata dfs** are built column-wise from the SoA store, not via
  `ToPandasEntry`. New helper `metadata_store_to_dataframe(store)`
  walks the fields of the store and emits columns directly.
- **Long-format dfs** (`parameters`, `provenance`, `removed_reasons`)
  are built by flattening the SoA `FnvHashMap` into parallel vectors
  and constructing a DataFrame.
- **Series accessors** wrap each Rust SoA's `BTreeMap<ID, T>` into a
  `pandas.Series[ID -> Object]` of Attached wrappers — one Python list
  of wrappers + one Index of IDs, no per-row dict allocation.

### `subscripts` / `provenance` representation

- `subscripts`: `List<Int64>` column on the metadata df (one row per
  id). Part of the variable / constraint identity, consumed as a
  tuple, not aggregated cross-row.
- `provenance`: long format `(id, step, source_kind, source_id)` — one
  row per `(id, step)` pair.

## Breaking changes

User-visible breakage relative to v3 alpha 2:

- `instance.constraints`, `decision_variables`, `*_constraints` change
  type from `dict` / `list` to `pandas.Series`. Most usage (`s.loc[id]`,
  iteration, `.items()`, `.index`) keeps working. Specific breakage:
  - `s.values()` (method call) → `s.tolist()` or `list(s)`.
  - List-positional reliance on the old `decision_variables: list[…]`
    ordering breaks; index by `VariableID` instead.
- `*_df` methods (`constraints_df(kind=...)`,
  `decision_variables_df()`, and the Solution / SampleSet
  counterparts) no longer carry `name`, `subscripts`, `description`,
  or `parameters.{key}` columns. Users join the relevant metadata /
  parameters df explicitly or call `*_full_df` for the joined form.
- New methods on `Instance`, `Solution`, and `SampleSet`:
  `constraint_metadata_df(kind=...)`,
  `constraint_parameters_df(kind=...)`,
  `constraint_provenance_df(kind=...)`,
  `constraint_removed_reasons_df(kind=...)`,
  `variable_metadata_df()`,
  `variable_parameters_df()`,
  plus `constraints_full_df(kind=...)` /
  `decision_variables_full_df()` joined helpers.
- Per-kind `indicator_constraints_df()`,
  `one_hot_constraints_df()`, `sos1_constraints_df()`, and
  `removed_*_constraints_df()` collapse into the single
  `constraints_df(kind=...)` overload set; same for the corresponding
  removed-reasons methods.
- The Rust `metadata` field on `DecisionVariable` and `Constraint<S>`
  is removed. Downstream Rust crates that touched `c.metadata.*`
  directly switch to `collection.metadata()` accessors.
- Wrapper-object metadata getters (`.name`, `.subscripts`, …) are
  **preserved**; they switch from owned data to back-reference reads.
  No user-visible change in the getter API.

A new section in `PYTHON_SDK_MIGRATION_GUIDE.md` covers the Python side
in detail.

## Open questions

Each item lists a working recommendation; flagged for explicit sign-off
before implementation.

1. **Constraint kind dispatch in Python**:
   `constraint_metadata_df(kind="regular")` (one method,
   `Literal["regular","indicator","one_hot","sos1"]` overloads) vs.
   `regular_constraint_metadata_df()` (four explicit methods).
   - **Recommendation: single method with `Literal` + `@overload`.**
     `pyo3-stub-gen` supports emitting `typing.overload` stubs keyed
     on `Literal[…]` arguments, so a `kind` parameter gives the same
     IDE / type-checker discoverability as four explicit methods
     while keeping the API surface compact. Each kind's overload
     advertises its specific column schema (e.g. only the regular and
     indicator overloads include the `provenance` / `function_type`
     columns where relevant).
2. **`removed_reason` placement**: inline columns on the core df vs. a
   separate long-format `*_removed_reasons_df`.
   - **Recommendation: separate long-format df.** `Solution` already
     exposes `removed_reasons_df` / `indicator_removed_reasons_df`
     this way, and `RemovedReason` is collection-level metadata in
     Rust, not part of the constraint.
3. **Builder-style metadata setter on the Rust side**: flat
   (`collection.insert(c)` followed by `metadata_mut().set_name(id,
   ...)`) only, vs. also `collection.insert_with(c, |m|
   m.name(...))` sugar.
   - **Recommendation: add `insert_with`.** Pure Rust callers
     (algorithms, adapters, tests) construct constraints in loops
     where the two-step form is easy to forget — the constraint goes
     in but the metadata silently doesn't. The closure-based form
     keeps insertion atomic at the call site. This is independent of
     the Python staging bag, which serves the modeling chain on the
     Python side; both surfaces exist for their own ergonomics
     reasons.
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
7. **`drop_constraint` / wrapper invalidation**: Attached wrappers
   stay valid as long as the id is in either the active or removed
   map. There's no `drop_constraint` API today, so the question is
   moot — but if one is added later, it has to invalidate Attached
   wrappers (panicking getters or `IsDroppedError`).
   - **Recommendation: do not add `drop_constraint` in v3.** `relax`
     is sufficient for the existing use cases. Defer wrapper
     invalidation semantics to whenever `drop_constraint` actually
     becomes necessary.
8. **Attached wrapper `Py<Instance>` cycles**: the wrapper holds a
   handle to the Instance; the Instance's collections own the SoA
   store. There's no cycle (wrapper → Instance, Instance → store, no
   back-pointer from store to wrapper), but heavy use of Series can
   keep an Instance alive longer than expected.
   - **Recommendation: documented behavior, no code-level mitigation.**
     Users who care about lifetime drop the Series.
