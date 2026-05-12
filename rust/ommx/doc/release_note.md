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
  `Instance`, `ParametricInstance`, `Solution`, and `SampleSet`;
  because the `id` field is gone from individual constraints, the
  **host is the natural unit of serialization**
  (`Instance::to_bytes`, `ParametricInstance::to_bytes`,
  `Solution::to_bytes`, `SampleSet::to_bytes`).
- Metadata (`name`, `subscripts`, `parameters`, `description`, plus
  `provenance` on constraints) moves off each constraint, decision
  variable, and named function into per-collection **Struct-of-Arrays
  metadata stores**, queried through narrow per-host accessors
  (`instance.constraint_metadata()`, `instance.variable_metadata()`,
  `instance.named_function_metadata()`, …). One canonical store per
  collection, two views on top: per-id wrapper getters for one-off
  reads and `*_df` for bulk analysis.
- A **capability model** lets adapters declare what they natively
  support and auto-converts unsupported kinds at the boundary, so a
  valid OMMX instance can be fed to any adapter (the conversion path
  is fallible and surfaces as `Err(ommx::Error)`, e.g. on non-finite
  bounds in the Big-M encoding).
- The default **error surface** is a single
  [`ommx::Result<T>`](crate::Result) (re-export of `anyhow::Result`)
  with diagnostic context emitted through `tracing` from the new
  fail-site macros. A handful of typed signal types
  (`BoundError`, `DecisionVariableError`, `DuplicatedSampleIDError`,
  `SubstitutionError`, …) stay typed at their public-API entry points
  for callers that recover by discriminant.
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

Promoting indicator / one-hot / SOS1 to first-class types alongside
regular `Constraint` would have multiplied the number of concrete
constraint structs by the number of lifecycle states each kind can be
in (created / evaluated / sampled, with removal handled separately).
Hand-writing the resulting 4 × 3 = 12 concrete struct definitions
wasn't going to scale.

The core refactor of 3.0 collapses the matrix: every constraint kind
is one generic struct parameterized by a [`Stage`](crate::Stage)
marker, and the [`ConstraintType`](crate::ConstraintType) trait names
the concrete `Created` / `Evaluated` / `Sampled` types per kind so
generic code (iteration, feasibility checks, DataFrame rendering,
adapter conversion) is written once and applied uniformly. The
[`EvaluatedConstraintBehavior`](crate::EvaluatedConstraintBehavior) /
[`SampledConstraintBehavior`](crate::SampledConstraintBehavior) traits
expose the per-kind feasibility surface in the same style.

Two knock-on simplifications:

- **No `Removed` stage.** Removal is collection-level state — see the
  next section.
- **No `id` field on the struct.** A constraint's ID lives on the
  enclosing `BTreeMap` key, which was already the single source of
  truth.

The migration guide's [Constraint Field Access](crate::doc::migration_guide#1-constraint-field-access)
and [New Types](crate::doc::migration_guide#new-types) sections cover
the struct shapes and the trait family in full.

## Collections and serialization ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#806](https://github.com/Jij-Inc/ommx/pull/806))

A trio of generic collection wrappers holds constraints uniformly
across every kind and every stage:
[`ConstraintCollection<T>`](crate::ConstraintCollection) on
`Instance` / `ParametricInstance` (active + removed paired with a
[`RemovedReason`](crate::RemovedReason)),
[`EvaluatedCollection<T>`](crate::EvaluatedCollection) on
[`Solution`](crate::Solution) (evaluated + removed-reason map), and
[`SampledCollection<T>`](crate::SampledCollection) on
[`SampleSet`](crate::SampleSet) (per-sample variants). Iteration,
feasibility checks, and `RemovedReason` handling work the same way
across every constraint kind at every stage, so adapter / Solution /
SampleSet code doesn't need to special-case the four kinds.

**Serialization moves to the host level.** With the `id` field gone
from individual constraints and metadata living in a per-collection
SoA store (next section), a single element can no longer round-trip
on its own. Per-element `to_bytes` / `from_bytes` are not provided on
any constraint kind or its evaluated / sampled counterpart; use
`Instance::to_bytes` / `from_bytes`,
`ParametricInstance::to_bytes` / `from_bytes`,
`Solution::to_bytes` / `from_bytes`, or
`SampleSet::to_bytes` / `from_bytes` as the entry points — each
encodes every constraint kind together with IDs and metadata in one
`v1::*` protobuf message.

The migration guide's [ConstraintCollection](crate::doc::migration_guide#constraintcollection)
and [EvaluatedCollection / SampledCollection](crate::doc::migration_guide#evaluatedcollection--sampledcollection)
reference cards list the public methods on each.

## Metadata storage: SoA store on the enclosing collection ([#843](https://github.com/Jij-Inc/ommx/pull/843), [#848](https://github.com/Jij-Inc/ommx/pull/848), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#853](https://github.com/Jij-Inc/ommx/pull/853))

Constraints, decision variables, and named functions used to carry
their metadata (`name`, `subscripts`, `parameters`, `description`, and
— for constraints only — `provenance`) inline on each element. In v3
the same fact lives in **one canonical place per collection**: a
Struct-of-Arrays metadata store keyed by ID, riding alongside the
constraint / variable / named-function map. Per-element structs shrink
to their intrinsic data and the SoA store is the canonical source for
both per-id reads and bulk DataFrame analysis.

Three store families share one shape:
[`ConstraintMetadataStore<ID>`](crate::ConstraintMetadataStore) on
every constraint-kind collection (the same store rides through
[`EvaluatedCollection<T>`](crate::EvaluatedCollection) and
[`SampledCollection<T>`](crate::SampledCollection) so metadata is
available at every stage),
[`VariableMetadataStore`](crate::VariableMetadataStore) as a sibling
field on `Instance` / `ParametricInstance` / `Solution` / `SampleSet`
(no separate `DecisionVariableCollection` was introduced), and
[`NamedFunctionMetadataStore`](crate::NamedFunctionMetadataStore) the
same way for named functions.

The split lets the type system enforce invariants more tightly. The
raw active/removed map mutators on `ConstraintCollection<T>` are
`pub(crate)` and `Instance` never hands out
`&mut ConstraintCollection<T>`, so external callers must go through
the validating `Instance::add_*` / `relax_*` / `restore_*` family —
which keep variable-id validity (every `id` referenced by a constraint
exists in `decision_variables`) and active/removed disjointness as
crate-internal invariants. Metadata mutation rides on its own `_mut()`
accessor and can't break either.

The Python side wraps the same SoA store with two parallel
user-facing changes:

- `instance.constraints[id]` and the parallel constraint / variable
  accessors return live `AttachedX` write-through handles
  ([#849](https://github.com/Jij-Inc/ommx/pull/849),
  [#850](https://github.com/Jij-Inc/ommx/pull/850),
  [#852](https://github.com/Jij-Inc/ommx/pull/852)).
- `*_df()` methods (with `kind=` / `include=` / `removed=` parameters)
  plus six long-format sidecar DataFrames serve bulk analysis directly
  off the SoA store ([#846](https://github.com/Jij-Inc/ommx/pull/846),
  [#847](https://github.com/Jij-Inc/ommx/pull/847)).

See [`PYTHON_SDK_MIGRATION_GUIDE.md`](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md)
§9–11 for the user-facing version.

The migration guide's [Metadata stores](crate::doc::migration_guide#metadata-stores)
section has the per-host accessor list and the store API reference.

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
solver, so an adapter can accept any valid OMMX `Instance` without
asking users to manually encode special constraints themselves. The
conversion is fallible — e.g. the Big-M encoding requires finite
bounds — and returns `Err(ommx::Error)` when the instance can't be
reduced; otherwise each conversion is emitted as an INFO-level
`tracing` event in the `reduce_capabilities` span for observability.

## Unified error surface ([#832](https://github.com/Jij-Inc/ommx/pull/832))

The default error type is [`ommx::Result<T>`](crate::Result) /
[`ommx::Error`](crate::Error), re-exports of `anyhow::Result<T>` and
`anyhow::Error` so downstream crates can propagate with `?` without
taking an `anyhow` dependency themselves. New crate-level fail-site
macros — [`bail!`](crate::bail) / [`error!`](crate::error!) /
[`ensure!`](crate::ensure) — emit a `tracing::error!` event alongside
producing the `anyhow::Error`, so diagnostic context lands in the
configured tracing subscriber rather than being stacked via
`anyhow::Error::context(...)` at the fail site.

The previous discriminant-style error enums (`InstanceError`,
`MpsParseError`, `StateValidationError`, `LogEncodingError`,
`UnknownSampleIDError`, the variants of `QplibParseError`, …) have
been removed — downstream code never matched on their variants in
practice. A handful of typed surfaces are deliberately kept:

- A curated set of **signal types** ([`InfeasibleDetected`](crate::InfeasibleDetected),
  [`BoundError`](crate::BoundError),
  [`DecisionVariableError`](crate::DecisionVariableError),
  [`DuplicatedSampleIDError`](crate::DuplicatedSampleIDError),
  [`SubstitutionError`](crate::SubstitutionError), …) returned typed
  by their entry-point APIs (`Bound::new`, `Sampled::append`,
  `Substitute::*`, …) for callers that recover by discriminant.
- Two narrow-domain parser errors that carry *positional* breadcrumbs
  ([`ParseError`](crate::ParseError),
  [`qplib::QplibParseError`](crate::qplib::QplibParseError)),
  converted to `ommx::Error` at the domain boundary.

The curated signal-type list and downcast / propagation patterns live
in the [error handling tutorial](crate::doc::tutorial::error_handling).

## Tracing-first observability ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#826](https://github.com/Jij-Inc/ommx/pull/826))

Internal logging has moved off `log` to `tracing`, and span coverage has been
broadened across parsing, evaluation, substitution, and solver adapter
entry points. Subscribers (including `tracing-opentelemetry`) pick up
structured fields and span context directly from the crate. New
internal fail sites use the crate-level `bail!` / `error!` / `ensure!`
macros instead of stacking context via `anyhow::Error::context(...)`,
so diagnostic information lands as a `tracing::error!` event at the
moment it's produced rather than being attached to the error chain
(legacy `.context(...)` call sites in narrow-domain parsers and the
artifact layer remain).

## Domain types replace `v1_ext` ([#799](https://github.com/Jij-Inc/ommx/pull/799), [#801](https://github.com/Jij-Inc/ommx/pull/801), [#803](https://github.com/Jij-Inc/ommx/pull/803), [#804](https://github.com/Jij-Inc/ommx/pull/804))

The proto-generated `v1::Instance` / `v1::Constraint` / `v1::Function` types
are now reserved for wire-format interop. All in-memory operations — QUBO/HUBO
conversions, slack helpers, relaxation, propagation, evaluation — are defined
on the domain types ([`Instance`](crate::Instance),
[`Constraint`](crate::Constraint), [`Function`](crate::Function), …) in the
crate root. The `v1_ext` helper module has been removed.

Two new domain traits accompany this shift:

- [`Propagate`](crate::Propagate) performs unit-propagation-style constraint
  reasoning and returns a [`PropagateOutcome`](crate::PropagateOutcome) —
  `Active(T)` (constraint shrunk in place), `Consumed(T)` (fully determined
  by the state, move to removed), or `Transformed { original, new }` (kind
  change, e.g. an indicator constraint promoted to a regular constraint).
  Callers that need provenance bookkeeping append a `Provenance` entry to
  the host's [`ConstraintMetadataStore`](crate::ConstraintMetadataStore)
  when they apply the outcome.
- [`Substitute`](crate::Substitute) performs symbolic variable substitution,
  with an acyclic fast path and full cycle detection.

## `ommx::artifact::ImageRef` replaces `ocipkg::ImageName`

`ImageRef` is the OMMX-owned parsed form of an OCI image reference
(`host[:port]/name:tag` or `host[:port]/name@sha256:...`). It supersedes
the previously re-exported `ocipkg::ImageName` on every public surface:
[`LocalArtifact::open`](crate::artifact::LocalArtifact::open),
[`LocalArtifactBuilder::new`](crate::artifact::LocalArtifactBuilder::new),
the SQLite Local Registry helpers, and the CLI parse path all now take
[`ImageRef`](crate::artifact::ImageRef). The accessors mirror what
`ImageName` offered (`hostname()`, `port()`, `name()`, `reference()`,
plus the v2-cache-compatible `as_path()` / `from_path()`), but field
access (`image_name.hostname`) becomes a method call. The
`ommx::ocipkg` re-export is removed.

## Other notable changes

- `ommx-derive` introduces `#[derive(LogicalMemoryProfile)]` for structural
  memory profiling
  ([#800](https://github.com/Jij-Inc/ommx/pull/800)).
- `ommx::doc` is now the entry point on docs.rs for long-form prose
  (this page, the [migration guide](crate::doc::migration_guide), and the
  [tutorial](crate::doc::tutorial)).
