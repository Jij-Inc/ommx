# Release Notes

Release notes for the `ommx` crate, covering the 3.0 line.

## 3.0

The 3.0.0 line is a major revision of the Rust SDK:

- **Indicator / one-hot / SOS1** are first-class constraint types alongside
  regular `Constraint`, with their own ID types and collection slots on
  `Instance`.
- The resulting constraint-kind × lifecycle combinatorics is tamed by a
  shared [`ConstraintType`](crate::ConstraintType) abstraction and a
  [`Stage`](crate::Stage) type parameter, so each kind is one generic
  struct rather than four hand-written ones.
- Constraints are grouped into generic
  [`ConstraintCollection<T>`](crate::ConstraintCollection) /
  [`EvaluatedCollection<T>`](crate::EvaluatedCollection) /
  [`SampledCollection<T>`](crate::SampledCollection) wrappers on
  `Instance`, `Solution`, and `SampleSet`; because the `id` field is
  gone from individual constraints, the **collection is the natural
  unit of serialization** (`Instance::to_bytes`, `Solution::to_bytes`,
  `SampleSet::to_bytes`).
- Metadata (`name`, `subscripts`, `parameters`, `description`,
  `provenance`) moves off each constraint and into per-collection
  **Struct-of-Arrays metadata stores**, queried through narrow
  per-host accessors (`instance.constraint_metadata()`,
  `instance.variable_metadata()`, …). One canonical store per
  collection, two views on top: per-id wrapper getters for one-off
  reads and `*_df` for bulk analysis.
- A **capability model** lets adapters declare what they natively support
  and auto-converts unsupported kinds at the boundary, so any OMMX
  instance can be fed to any adapter.
- The public **error surface** collapses to a single type, with diagnostic
  context emitted through `tracing` rather than stacked via
  `anyhow::Context`.
- The long-running migration away from the proto-generated `v1_ext`
  helpers finishes: domain types (`Instance`, `Constraint`,
  `DecisionVariable`, …) are the primary API, and `v1::*` is reserved for
  wire-format interop.

See the [migration guide](crate::doc::migration_guide) for the detailed
v2 → v3 upgrade path. This page is a topic-oriented summary of what changed and
why.

## First-class special constraint types ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#798](https://github.com/Jij-Inc/ommx/pull/798))

Special-structure constraints are now first-class domain objects, parallel to
the regular [`Constraint`](crate::Constraint) rather than metadata hanging off
it:

- [`IndicatorConstraint`](crate::IndicatorConstraint) — encoding
  `indicator_variable = 1 → f(x) {=,≤} 0`. **New in v3.**
- [`OneHotConstraint`](crate::OneHotConstraint) — exactly one of a set of
  binary variables is 1. Previously expressed as a
  `ConstraintHints::OneHot` hint on a regular equality constraint; now a
  constraint type in its own right.
- [`Sos1Constraint`](crate::Sos1Constraint) — at most one of a set of
  variables is non-zero. Previously `ConstraintHints::Sos1`; now first-class.

Each kind has its own ID type
([`IndicatorConstraintID`](crate::IndicatorConstraintID),
[`OneHotConstraintID`](crate::OneHotConstraintID),
[`Sos1ConstraintID`](crate::Sos1ConstraintID)) to prevent accidental
cross-type lookups, and lives in its own collection slot on
[`Instance`](crate::Instance).

## Stage parameter and the `ConstraintType` trait ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796), [#806](https://github.com/Jij-Inc/ommx/pull/806))

Promoting indicator / one-hot / SOS1 to first-class types alongside regular
`Constraint` multiplies the number of concrete constraint structs by the
number of lifecycle states each kind can be in — created, evaluated,
sampled, and (before v3) removed. Hand-writing four-kind × four-state = 16
concrete struct definitions was never going to scale; the core refactor of
3.0.0 is the abstraction that collapses that matrix.

Every constraint kind is now a single generic struct parameterized by a
[`Stage`](crate::Stage) marker. For the regular constraint:

```rust,ignore
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub stage: S::Data,
}
```

with three inhabited stages — `Created`, `Evaluated`, and `Sampled`. Each
stage swaps in different `stage` data (the function for `Created`, the
evaluated value and feasibility for `Evaluated`, per-sample vectors for
`Sampled`). The type aliases `EvaluatedConstraint = Constraint<Evaluated>`
and `SampledConstraint = Constraint<stage::Sampled>` keep the common
names as entry points. `IndicatorConstraint`, `OneHotConstraint`, and
`Sos1Constraint` share the same `Stage` / `ConstraintType` pattern,
though their `Created`-stage data differs (an `indicator_variable` on
`IndicatorConstraint`, a `variables` set on `OneHotConstraint` /
`Sos1Constraint`).

The unifying abstraction is the
[`ConstraintType`](crate::ConstraintType) trait, a defunctionalization of
`Stage → Type` that names each kind's concrete stage types:

```rust,ignore
pub trait ConstraintType {
    type ID;
    type Created;     // e.g. Constraint, IndicatorConstraint, …
    type Evaluated;   // e.g. EvaluatedConstraint, EvaluatedIndicatorConstraint, …
    type Sampled;     // e.g. SampledConstraint, SampledIndicatorConstraint, …
}
```

`ConstraintCollection<T>` / `EvaluatedCollection<T>` / `SampledCollection<T>`
(used by `Instance`, `Solution`, and `SampleSet` respectively) are
parameterized by `T: ConstraintType`, so generic code — iteration,
feasibility checks, DataFrame rendering, adapter conversion — is written
once and applied uniformly across every constraint kind. The
[`EvaluatedConstraintBehavior`](crate::EvaluatedConstraintBehavior) and
[`SampledConstraintBehavior`](crate::SampledConstraintBehavior) traits
expose the per-kind feasibility surface in the same style.

Two knock-on simplifications fall out:

- **No `Removed` stage.** Removal is collection-level state, not a
  stage (see "Collections and serialization" below).
- **No `id` field on the struct.** The constraint's ID lives on the
  enclosing `BTreeMap<T::ID, T::Created>` key, which was already the
  single source of truth, so standalone constraints are identity-less
  until inserted into a collection.

## Collections and serialization ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#806](https://github.com/Jij-Inc/ommx/pull/806))

The trait above is only half the story. The other half is a trio of
generic collection wrappers that hold constraints uniformly across every
kind and every stage:

- [`ConstraintCollection<T>`](crate::ConstraintCollection) — active
  constraints plus removed ones paired with a
  [`RemovedReason`](crate::RemovedReason). Used by
  [`Instance`](crate::Instance); replaces the old flat
  `Instance.constraints` + `Instance.removed_constraints` fields with
  one typed slot per constraint kind
  (`constraint_collection()`, `indicator_constraint_collection()`,
  `one_hot_constraint_collection()`, `sos1_constraint_collection()`).
  Exposes `active()` / `removed()`, `relax(id, reason)` /
  `restore(id)`, and `required_ids()`.
- [`EvaluatedCollection<T>`](crate::EvaluatedCollection) — evaluated
  constraints plus a map of `RemovedReason`s for any that were relaxed
  before evaluation. Used by [`Solution`](crate::Solution). Exposes
  `is_feasible()` / `is_feasible_relaxed()` / `removed_reasons()` /
  `is_removed(&id)`.
- [`SampledCollection<T>`](crate::SampledCollection) — sampled
  constraints plus the corresponding `RemovedReason`s. Used by
  [`SampleSet`](crate::SampleSet). Exposes `is_feasible_for(sample_id)`
  / `is_feasible_relaxed_for(sample_id)` / `removed_reasons()` /
  `is_removed(&id)` — the per-sample variants of the `Evaluated`
  versions, since feasibility is decided per draw.

Iterating and RemovedReason handling work the same way across every
kind at each stage, so code on the adapter / Solution / SampleSet side
doesn't need to special-case `Constraint` vs `IndicatorConstraint` vs
`OneHotConstraint` vs `Sos1Constraint`.

**Serialization moves to the collection level.** Because constraints no
longer carry their own `id` field, the natural unit of serialization is
the enclosing collection (which owns the `ConstraintID → Constraint`
mapping). Concretely:

- [`Instance::to_bytes`](crate::Instance::to_bytes) /
  [`from_bytes`](crate::Instance::from_bytes), `Solution::to_bytes` /
  `from_bytes`, and `SampleSet::to_bytes` / `from_bytes` are the
  recommended entry points. Each encodes every constraint kind together
  with its IDs in one `v1::*` protobuf message.
- Per-constraint `to_bytes` / `from_bytes` on the regular
  `Constraint`, `EvaluatedConstraint`, and `SampledConstraint` still
  exist but now require an explicit `ConstraintID` argument and return
  `(ConstraintID, T)` on decode — the type can no longer round-trip on
  its own.
- The new special-constraint types (`IndicatorConstraint`,
  `OneHotConstraint`, `Sos1Constraint`) intentionally have no
  per-constraint `to_bytes` / `from_bytes`; they are only serialized as
  part of an `Instance`, `Solution`, or `SampleSet`.

[`ParametricInstance`](crate::ParametricInstance) follows the same
shape: the same typed collection slots per constraint kind, and its
own `to_bytes` / `from_bytes` at the instance level.

## Metadata storage: SoA store on the enclosing collection ([#843](https://github.com/Jij-Inc/ommx/pull/843), [#846](https://github.com/Jij-Inc/ommx/pull/846), [#847](https://github.com/Jij-Inc/ommx/pull/847), [#848](https://github.com/Jij-Inc/ommx/pull/848), [#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852), [#853](https://github.com/Jij-Inc/ommx/pull/853))

Constraints, decision variables, and named functions used to carry
their metadata inline. In v3 the same fact lives in **one canonical
place per collection** — a Struct-of-Arrays metadata store keyed by
ID — and per-element structs shrink to their intrinsic data:

```rust,ignore
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub stage: S::Data,
    // metadata field removed
}

pub struct ConstraintCollection<T: ConstraintType> {
    active:   BTreeMap<T::ID, T::Created>,
    removed:  BTreeMap<T::ID, (T::Created, RemovedReason)>,
    metadata: ConstraintMetadataStore<T::ID>,   // new
}
```

Three store families share one shape (`name` / `subscripts` /
`parameters` / `description`, plus `provenance` on constraints):

- [`ConstraintMetadataStore<ID>`](crate::ConstraintMetadataStore) on
  every [`ConstraintCollection<T>`](crate::ConstraintCollection),
  [`EvaluatedCollection<T>`](crate::EvaluatedCollection), and
  [`SampledCollection<T>`](crate::SampledCollection) — so the same
  metadata source rides through evaluation and sampling.
- [`VariableMetadataStore`](crate::VariableMetadataStore) as a sibling
  field on `Instance` / `ParametricInstance` / `Solution` / `SampleSet`
  (no separate `DecisionVariableCollection` was introduced).
- [`NamedFunctionMetadataStore`](crate::NamedFunctionMetadataStore) the
  same way for named functions.

Per-host accessors expose them safely: `instance.constraint_metadata()`,
`indicator_constraint_metadata()`, `one_hot_constraint_metadata()`,
`sos1_constraint_metadata()`, `variable_metadata()`,
`named_function_metadata()` (each with a `_mut()` companion). The store
itself offers per-field borrowing reads (`name(id) -> Option<&str>`,
`subscripts(id) -> &[i64]`, …), a one-shot owned reconstruction
(`collect_for(id) -> ConstraintMetadata`), and write-through setters
(`set_name`, `push_subscript`, `set_parameter`, `push_provenance`, …).
Bulk owned exchange via `insert(id, ConstraintMetadata)` /
`remove(id) -> ConstraintMetadata` keeps the existing
[`ConstraintMetadata`](crate::ConstraintMetadata) struct viable as the
I/O / modeling-input shape.

The split tightens the invariants: variable-id validity is an
`Instance`-level property (every `id` referenced by a constraint must
live in `decision_variables`), while active/removed disjointness is a
`ConstraintCollection`-level property. `Instance` never hands out a
`&mut ConstraintCollection<T>` — the raw active/removed map mutators
(`active_mut`, `removed_mut`, `insert_with`) are `pub(crate)`, so
external callers can only mutate constraint membership through the
validating `Instance::add_*` / `relax_*` / `restore_*` family.
Per-host metadata mutation goes through the `_mut()` accessor on the
SoA store, which can't break either invariant.

On the Python side this drives a parallel set of changes:
- `instance.constraints[id]` etc. return write-through
  [`AttachedX`](https://github.com/Jij-Inc/ommx/pull/849) handles whose
  reads pull live from the SoA store and whose metadata setters write
  back through to it. The snapshot wrapper types
  (`Constraint`, `IndicatorConstraint`, …) remain as the modeling-input
  shape, and `attached.detach()` materializes a snapshot when needed.
- `*_df` accessors are methods, with `kind=` /
  `include=("metadata","parameters","removed_reason")` /
  `removed=` parameters consolidating the old per-kind families. Six
  long-format sidecar DataFrames (`constraint_metadata_df`,
  `constraint_parameters_df`, `constraint_provenance_df`,
  `constraint_removed_reasons_df`, `variable_metadata_df`,
  `variable_parameters_df`) read directly from the stores for tidy-data
  joins.

See the [migration guide](crate::doc::migration_guide#metadata-stores) for the per-host accessor reference and call-site rewrites.

## Capability model ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

First-class special constraints raise a deployment question: not every
solver supports indicator / one-hot / SOS1 natively, and some only
support a subset. v3 answers this with an explicit capability model
built into the domain layer.

[`AdditionalCapability`](crate::AdditionalCapability) is an enum of the
non-standard constraint kinds, and [`Capabilities`](crate::Capabilities)
is a sorted set of them. Two `Instance` methods anchor the model:

- [`Instance::required_capabilities()`](crate::Instance::required_capabilities)
  returns the capabilities an instance actually uses (only active
  constraints are considered; removed constraints are excluded since
  they are never passed to the solver).
- [`Instance::reduce_capabilities(&supported)`](crate::Instance::reduce_capabilities)
  takes the capabilities a given solver can handle and converts every
  unsupported kind into regular constraints via the canonical encodings
  (Big-M for indicator / SOS1, linear equality for one-hot). On return,
  `required_capabilities()` is a subset of `supported`.

Adapters declare what they natively support and call
`reduce_capabilities` before handing the instance to the underlying
solver, so an adapter can accept any OMMX `Instance` without asking
users to manually encode special constraints themselves. Each conversion
is also emitted as an INFO-level `tracing` event in the
`reduce_capabilities` span for observability.

## Unified error surface ([#832](https://github.com/Jij-Inc/ommx/pull/832))

The public API of the crate now returns a single error type.
[`ommx::Result<T>`](crate::Result) and [`ommx::Error`](crate::Error) are
re-exports of `anyhow::Result<T>` and `anyhow::Error`, so downstream crates can
propagate with `?` without taking an `anyhow` dependency themselves.

The previous discriminant-style error enums (`InstanceError`, `MpsParseError`,
`StateValidationError`, `LogEncodingError`, `UnknownSampleIDError`, the
variants of `QplibParseError`, …) have been removed — downstream code never
matched on their variants in practice, so the enums were pure ceremony.
A small set of **signal types** remains `pub` because callers do recover them
by downcast: [`InfeasibleDetected`](crate::InfeasibleDetected),
[`CoefficientError`](crate::CoefficientError),
[`BoundError`](crate::BoundError), [`AtolError`](crate::AtolError),
[`DecisionVariableError`](crate::DecisionVariableError),
[`SubstitutionError`](crate::SubstitutionError),
[`SolutionError`](crate::SolutionError),
[`SampleSetError`](crate::SampleSetError), and
[`DuplicatedSampleIDError`](crate::DuplicatedSampleIDError).

Two narrow-domain parsers keep their structured error types because they carry
*positional* breadcrumbs that editors and diagnostic UIs can consume:
[`ParseError`](crate::ParseError) (proto-tree `Vec<ParseContext>`) and
[`qplib::QplibParseError`](crate::qplib::QplibParseError) (1-based line number
and rendered message).

Diagnostic context flows through `tracing`, not through
`anyhow::Error::context(...)`. The fail-site macros
[`bail!`](crate::bail) / [`error!`](crate::error!) / [`ensure!`](crate::ensure)
emit a `tracing::error!` event alongside producing an `anyhow::Error` from the
same format string.

## Tracing-first observability ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#826](https://github.com/Jij-Inc/ommx/pull/826))

Internal logging has moved off `log` to `tracing`, and span coverage has been
broadened across parsing, evaluation, substitution, and solver adapter
entry points. Subscribers (including `tracing-opentelemetry`) pick up
structured fields and span context directly from the crate — no ad-hoc context
stacking via `anyhow::Error::context(...)` is needed.

## Domain types replace `v1_ext` ([#799](https://github.com/Jij-Inc/ommx/pull/799), [#801](https://github.com/Jij-Inc/ommx/pull/801), [#803](https://github.com/Jij-Inc/ommx/pull/803), [#804](https://github.com/Jij-Inc/ommx/pull/804))

The proto-generated `v1::Instance` / `v1::Constraint` / `v1::Function` types
are now reserved for wire-format interop. All in-memory operations — QUBO/HUBO
conversions, slack helpers, relaxation, propagation, evaluation — are defined
on the domain types ([`Instance`](crate::Instance),
[`Constraint`](crate::Constraint), [`Function`](crate::Function), …) in the
crate root. The `v1_ext` helper module has been removed.

Two new domain traits accompany this shift:

- [`Propagate`](crate::Propagate) performs unit-propagation-style constraint
  reasoning and returns a [`PropagateOutcome`](crate::PropagateOutcome) that
  records a `Provenance` chain for every substitution it applied.
- [`Substitute`](crate::Substitute) performs symbolic variable substitution,
  with an acyclic fast path and full cycle detection.

## Other notable changes

- `ommx-derive` introduces `#[derive(LogicalMemoryProfile)]` for structural
  memory profiling
  ([#800](https://github.com/Jij-Inc/ommx/pull/800)).
- `ommx::doc` is now the entry point on docs.rs for long-form prose
  (this page, the [migration guide](crate::doc::migration_guide), and the
  [tutorial](crate::doc::tutorial)).
