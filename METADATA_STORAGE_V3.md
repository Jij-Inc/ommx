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
- `*_df` methods are explicitly **derived views** with an `include`
  parameter that controls which sidecars are folded in. The default
  `include` keeps the v2-style wide DataFrame shape, so existing user
  code only needs to add the `kind=...` argument to keep working.
- Sidecar DataFrames are bulk-built from the Rust SoA store with one
  column allocation per field. They are still exposed individually
  (`constraint_metadata_df(kind=...)`,
  `constraint_parameters_df(kind=...)`,
  `constraint_provenance_df(kind=...)`,
  `constraint_removed_reasons_df(kind=...)`,
  `variable_metadata_df`, `variable_parameters_df`) for users who
  want long-format data, but the `*_df` family covers the common
  "give me a wide table" case directly.
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
  Series         Constraint object     constraints_df(kind=..., include=...) /
  (per-id        (with .name /         constraint_metadata_df(kind=...) /
   wrapper       .subscripts / …       constraint_parameters_df(kind=...) /
   handles)      back-referenced       constraint_provenance_df(kind=...) /
                 to the store)         constraint_removed_reasons_df(kind=...)
                                       (bulk-built from the SoA via column-wise
                                       builders; kind via Literal + @overload)
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
# corresponding constraints_df / constraint_metadata_df / etc.
# family with the same `include=` parameter, and back-referenced
# wrappers for individual elements. The (unrelated) parametric
# `parameters` field stays on its own dedicated accessor.
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

### `*_df` methods → derived views with `include`

Each `*_df` is a derived view: type-specific core columns extracted
from the SoA store, plus whichever sidecars the caller asks for via
`include`. The default `include` matches v2's wide-DataFrame shape
(`metadata` + `parameters`), so v2 user code keeps working with only
a `kind=...` argument added.

```python
# === v2-style wide DataFrame (default include) ===
df = instance.constraints_df(kind="regular")
# ≡ instance.constraints_df(kind="regular", include=("metadata", "parameters"))
# index name = regular_constraint_id
# columns: equality, function_type, used_ids,
#          name, subscripts, description, parameters.{key}, ...

df = instance.decision_variables_df()
# ≡ instance.decision_variables_df(include=("metadata", "parameters"))
# index name = variable_id
# columns: kind, lower, upper, substituted_value,
#          name, subscripts, description, parameters.{key}, ...

# === Core only — pass include=() ===
df = instance.constraints_df(kind="regular", include=())
# columns: equality, function_type, used_ids

df = instance.decision_variables_df(include=())
# columns: kind, lower, upper, substituted_value

# === Add removed_reasons (not in v2 default) ===
df = instance.constraints_df(
    kind="regular",
    include=("metadata", "parameters", "removed_reasons"),
)
# … plus removed_reasons.reason, removed_reasons.{key}

# === Long-format sidecars when wide pivoting isn't what you want ===
meta     = instance.constraint_metadata_df(kind="regular")
                                            # index name=regular_constraint_id;
                                            # name, subscripts, description
provs    = instance.constraint_provenance_df(kind="regular")
                                            # columns: regular_constraint_id, step,
                                            # source_kind, source_id (long format)
params   = instance.constraint_parameters_df(kind="regular")
                                            # columns: regular_constraint_id, key, value
removed  = instance.constraint_removed_reasons_df(kind="regular")
                                            # columns: regular_constraint_id, reason,
                                            # key, value
```

`include` accepts a tuple of `Literal["metadata","parameters","removed_reasons"]`
values. `provenance` is intentionally absent from `include`: chains
have variable length, so a wide pivot would either explode the column
space (`provenance.0.*`, `provenance.1.*`, …) or produce an
object-dtype list column. Users who want provenance pivot the long-
format `constraint_provenance_df()` themselves.

`Solution` and `SampleSet` expose the same `*_df` family with stage-
appropriate core-column schemas and the same `include` parameter:

```python
# Solution — evaluated stage; v2-style default
df = solution.constraints_df(kind="regular")
                                            # core: equality, evaluated_value, feasible,
                                            # used_ids, dual_variable
                                            # + metadata, parameters columns by default

df = solution.decision_variables_df()       # core: kind, lower, upper, value
                                            # + metadata, parameters columns by default

# SampleSet — sampled stage; per-sample core columns (value.{sid},
# feasible.{sid}, …); same include defaults
df = sample_set.constraints_df(kind="regular")
df = sample_set.decision_variables_df()
```

The metadata / parameters / provenance / removed-reasons sidecar dfs
are stage-independent — they describe the same constraint, so
`solution.constraint_metadata_df(kind=...)` returns the same data as
`instance.constraint_metadata_df(kind=...)` (and the `include=` columns
folded into `*_df` are identical across stages too). There's no per-
stage divergence to manage.

`*_df` (with `include=`) is what users call when they want a wide
table for analysis; the long-format sidecars are what they call when
they want tidy data for joins or aggregation; the Series is what they
call when they want individual wrapper objects; the wrapper getters
are what they call when they already hold one wrapper. Four surfaces,
one canonical store.

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

2. **`include=` covers the common "wide" case without manual join.**
   The `*_df` methods themselves accept an `include` parameter that
   folds sidecars into the result, so most users never write a
   `df.join(other_df)` at all:

   ```python
   instance.constraints_df(kind="regular")
   # default include=("metadata","parameters") — v2-equivalent shape

   instance.constraints_df(kind="regular",
                           include=("metadata","parameters","removed_reasons"))
   # core + metadata + pivoted parameters + pivoted removed_reasons
   ```

   `include` is a tuple of literal strings (typed via `Literal[...]`).
   `"metadata"` is left-joined as columns; `"parameters"` and
   `"removed_reasons"` are left-joined after pivoting their long-
   format keys back to wide columns under namespaced prefixes
   (`parameters.{key}`, `removed_reasons.{key}`). Cross-ID-space
   mistakes are impossible inside `include=` because the helper
   knows the right kind to look up internally. Users who need a
   manual cross-df `join` go through the long-format sidecars,
   where the qualified index names (point 1) make the mistake
   visible.

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
index name + `include=` covering the common wide case are sufficient.

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

- `subscripts`: a single `List<Int64>` value per id — exposed as a
  list column on `*_metadata_df` and as `wrapper.subscripts: list[int]`
  via the back-reference. **Not** offered in long format. `subscripts`
  is part of the variable / constraint identity (the "tuple index" in
  `x[i, j, k]`-style expressions), and exploding it across rows would
  invite treating positions as first-class entities — which is the
  wrong mental model. Users who genuinely want to filter on
  `subscripts[0]` do it in Python (`df[df.subscripts.str[0] == i]`)
  rather than via a long-format API.
- `provenance`: long format `(id, step, source_kind, source_id)` — one
  row per `(id, step)` pair. Unlike `subscripts`, provenance steps
  *are* first-class entities (each step is one transformation event),
  and chains have variable length, so the long shape is the natural
  one.

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
  counterparts) gain an `include=` parameter selecting which
  sidecars to fold in. The default
  (`include=("metadata","parameters")`) reproduces the v2 wide-
  DataFrame shape, so v2 user code keeps working with only a
  `kind=...` argument added — `df["name"]`,
  `df["parameters.{key}"]`, etc. continue to resolve. Users who
  want only the core columns pass `include=()`.
- New long-format sidecar methods on `Instance`, `Solution`, and
  `SampleSet`: `constraint_metadata_df(kind=...)`,
  `constraint_parameters_df(kind=...)`,
  `constraint_provenance_df(kind=...)`,
  `constraint_removed_reasons_df(kind=...)`,
  `variable_metadata_df()`, `variable_parameters_df()`. These
  expose the SoA store as tidy long-format DataFrames for users
  who want pivot / aggregation / cross-instance union beyond what
  `include=` covers.
- Per-kind `indicator_constraints_df()`,
  `one_hot_constraints_df()`, `sos1_constraints_df()`, and
  `removed_*_constraints_df()` collapse into the single
  `constraints_df(kind=...)` overload set.
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
   - **Recommendation: reject.** `subscripts` is part of the
     variable / constraint identity (the "tuple index" in expressions
     like `x[i, j, k]`), not a collection that users meaningfully
     iterate or aggregate over. Exposing it in long form would invite
     treating positions as first-class entities, which is the wrong
     mental model. Always serve `subscripts` as a single `List<Int64>`
     value per id — both on the metadata df and on the wrapper getter.
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
