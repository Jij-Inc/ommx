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
- Modeling labels (`name`, `subscripts`, `parameters`, `description`) move
  off each constraint, decision variable, and named function into
  per-collection **Struct-of-Arrays label/context stores**. Constraint
  `provenance` is kept in `ConstraintContext`, separate from the label.
  These stores are queried through narrow per-host accessors
  (`instance.constraint_context()`, `instance.variable_labels()`,
  `instance.named_function_labels()`, …). One canonical store per
  collection, two views on top: per-id wrapper getters for one-off
  reads and `*_df` for bulk analysis.
- Decision variables, parameters, and named functions now follow the same table ownership rule:
  [`DecisionVariable`](crate::DecisionVariable) is row data containing
  only `kind` and `bound`; the [`VariableID`](crate::VariableID),
  modeling labels, and fixed values live on
  lifecycle-stage-parameterized [`DecisionVariableTable`](crate::DecisionVariableTable)
  for `Instance` / `ParametricInstance`, while
  [`EvaluatedDecisionVariableTable`](crate::EvaluatedDecisionVariableTable)
  and [`SampledDecisionVariableTable`](crate::SampledDecisionVariableTable)
  are aliases for the evaluated and sampled stages on `Solution` / `SampleSet`.
  [`ParametricInstance`](crate::ParametricInstance) stores parameter IDs
  and labels in [`ParameterTable`](crate::ParameterTable), while concrete
  parameter values remain inputs to
  [`ParametricInstance::with_parameters`](crate::ParametricInstance::with_parameters).
  [`NamedFunction`](crate::NamedFunction) likewise stores only the
  [`Function`](crate::Function); [`NamedFunctionID`](crate::NamedFunctionID)
  lives on the enclosing named-function maps.
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
the regular [`Constraint`](crate::Constraint) rather than hints hanging off it:

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
from individual constraints and labels/context living in a per-collection
SoA store (next section), a single element can no longer round-trip
on its own. Per-element `to_bytes` / `from_bytes` are not provided on
any constraint kind or its evaluated / sampled counterpart; use
`Instance::to_bytes` / `from_bytes`,
`ParametricInstance::to_bytes` / `from_bytes`,
`Solution::to_bytes` / `from_bytes`, or
`SampleSet::to_bytes` / `from_bytes` as the entry points — each
encodes every constraint kind together with IDs and labels/context in one
`v1::*` protobuf message.

The migration guide's [ConstraintCollection](crate::doc::migration_guide#constraintcollection)
and [EvaluatedCollection / SampledCollection](crate::doc::migration_guide#evaluatedcollection--sampledcollection)
reference cards list the public methods on each.

## Modeling labels and constraint context on the enclosing collection ([#843](https://github.com/Jij-Inc/ommx/pull/843), [#848](https://github.com/Jij-Inc/ommx/pull/848), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#853](https://github.com/Jij-Inc/ommx/pull/853))

Constraints, decision variables, and named functions used to carry their
labels (`name`, `subscripts`, `parameters`, `description`) inline on each
element. In v3 the same modeling-label fact lives in **one canonical place
per collection**: a Struct-of-Arrays label/context store keyed by ID, riding
alongside the constraint / variable / named-function map. Constraint
`provenance` is part of `ConstraintContext`, not part of `ModelingLabel`.
Per-element structs shrink to their intrinsic data and the SoA store is the
canonical source for both per-id reads and bulk DataFrame analysis.

Three store families share one shape:
[`ConstraintContextStore<ID>`](crate::ConstraintContextStore) on
every constraint-kind collection (the same store rides through
[`EvaluatedCollection<T>`](crate::EvaluatedCollection) and
[`SampledCollection<T>`](crate::SampledCollection) so context is
available at every stage),
[`DecisionVariableTable`](crate::DecisionVariableTable), which owns
decision-variable definition rows, fixed values, and the
[`VariableLabelStore`](crate::VariableLabelStore) together on
`Instance` / `ParametricInstance`,
[`EvaluatedDecisionVariableTable`](crate::EvaluatedDecisionVariableTable) and
[`SampledDecisionVariableTable`](crate::SampledDecisionVariableTable), which
own decision-variable result rows and labels on `Solution` / `SampleSet`, and
[`NamedFunctionLabelStore`](crate::NamedFunctionLabelStore) inside
[`NamedFunctionTable`](crate::NamedFunctionTable), which owns named-function
rows and labels together.

The split lets the type system enforce invariants more tightly.
`ConstraintCollection<T>` no longer exposes raw active/removed/context map
mutation to `Instance` transformation code. Transformations go through
operation-level collection effects such as fresh active insertion with context,
active-row rewrites, lifecycle-preserving replacement, relax, and restore
through a host-supplied normalizer. External callers still go through the
validating `Instance::add_*` / `relax_*` / `restore_*` family, which keep
variable-id validity (every `id` referenced by a constraint exists in
`decision_variables`), active/removed disjointness, and label/provenance
sidecar ownership together.

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

See the [Python SDK v2 to v3 Migration Guide](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/migration/python_sdk_v2_to_v3.html)
§9–11 for the user-facing version.

The migration guide's [Modeling labels and constraint context](crate::doc::migration_guide#modeling-labels-and-constraint-context)
section has the per-host accessor list and the store API reference.

## Decision-variable table ownership ([#969](https://github.com/Jij-Inc/ommx/pull/969))

The Rust SDK now has explicit decision-variable table owners. The map key owns
the [`VariableID`](crate::VariableID), the row owns only intrinsic data, and the
table owns modeling labels. `Instance` and `ParametricInstance` use
[`DecisionVariableTable`](crate::DecisionVariableTable), which also owns fixed
values; `Solution` uses
[`EvaluatedDecisionVariableTable`](crate::EvaluatedDecisionVariableTable), and
`SampleSet` uses
[`SampledDecisionVariableTable`](crate::SampledDecisionVariableTable).
These are the same table owner parameterized by the shared lifecycle stages used
by constraints, so row IDs and modeling labels are validated by one
implementation while fixed values remain a created-stage column.

This removes the remaining duplicate ID source from the Rust-side row structs.
Construct `DecisionVariable` rows with `DecisionVariable::new(kind, bound, atol)`
or no-argument factories such as `DecisionVariable::binary()`, then insert them
under the desired `VariableID` key. The row still owns the `kind`/`bound`
invariant: safe construction and bound mutation normalize `bound` with the
caller-provided `ATol`. `EvaluatedDecisionVariable::new`
and `SampledDecisionVariable::new` still take the ID as a separate argument so
non-finite value errors can report the table key, but the resulting row data
does not store that ID.
For direct created-stage table construction, use
`DecisionVariableTable::with_fixed_values(entries, labels, fixed_values, atol)`.
An empty `fixed_values` map represents the same table schema with no fixed
rows; there is no separate empty-sidecar constructor.

The deprecated `Solution::new` constructor was removed. It was a safe API that
skipped host-level validation by wrapping `SolutionBuilder::build_unchecked`.
Use `Solution::builder().build()` for validated construction, or call the
unsafe `build_unchecked` path only when the enclosing code has already
guaranteed the `Solution` invariants.

This is part of the normalization work tracked in
[#958](https://github.com/Jij-Inc/ommx/issues/958).

## Named-function table ownership ([#964](https://github.com/Jij-Inc/ommx/pull/964))

Named functions now follow the same table-owned ID model as constraints and
decision variables. The Rust SDK row structs no longer carry their own
[`NamedFunctionID`](crate::NamedFunctionID):

- [`NamedFunction`](crate::NamedFunction) stores only the intrinsic
  [`Function`](crate::Function).
- [`EvaluatedNamedFunction`](crate::EvaluatedNamedFunction) stores the
  evaluated value and used decision-variable IDs.
- [`SampledNamedFunction`](crate::SampledNamedFunction) stores sampled values
  and used decision-variable IDs.

The ID, row map, and modeling labels now live together in
[`NamedFunctionTable`](crate::NamedFunctionTable). `Instance` and
`ParametricInstance` store `NamedFunctionTable<NamedFunction>`, `Solution`
stores `NamedFunctionTable<EvaluatedNamedFunction>`, and `SampleSet` stores
`NamedFunctionTable<SampledNamedFunction>`. Legacy `ommx.v1` protobuf messages
still carry inline IDs; Rust parse drains them into table keys and Rust
serialization fills them from table keys.

Mutable named-function row views are not exposed from host objects. In
particular, [`Instance::new_named_function`](crate::Instance::new_named_function)
now returns the allocated [`NamedFunctionID`](crate::NamedFunctionID) rather
than `&mut NamedFunction`, so callers cannot invalidate a checked `Instance` by
editing the function body after insertion.

## Parameter table ownership ([#967](https://github.com/Jij-Inc/ommx/pull/967))

[`ParametricInstance`](crate::ParametricInstance) now stores its parameter
universe as a [`ParameterTable`](crate::ParameterTable) rather than
`BTreeMap<VariableID, v1::Parameter>`. Parameter IDs intentionally stay in the
shared [`VariableID`](crate::VariableID) namespace with decision variables,
because [`Function`](crate::Function) references can only be interpreted as
decision variables or parameters by the enclosing `ParametricInstance`.

`ParameterTable` owns the parameter ID set and
[`ParameterLabelStore`](crate::ParameterLabelStore). It enforces the
table-level invariant that labels cannot reference unknown parameter IDs.
`ParametricInstance` remains responsible for the host-level invariants:
parameter IDs and decision-variable IDs must be disjoint, expressions may only
reference IDs from the combined namespace, and structural decision-variable
positions such as indicator / one-hot / SOS1 members cannot use parameter IDs.

Rust callers should pass `ParameterTable` to
[`ParametricInstance::new`](crate::ParametricInstance::new) and
[`ParametricInstanceBuilder::parameters`](crate::ParametricInstanceBuilder::parameters).
Legacy `ommx.v1.Parameter` rows are still used at protobuf and Python API
boundaries; Rust parsing drains their inline IDs and labels into
`ParameterTable`, and Rust serialization materializes them back from the table.

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
  the host's [`ConstraintContextStore`](crate::ConstraintContextStore)
  when they apply the outcome.
- [`Substitute`](crate::Substitute) performs symbolic variable substitution,
  with an acyclic fast path and full cycle detection.

## `ommx::artifact::ImageRef` replaces `ocipkg::ImageName`

`ImageRef` is the OMMX-owned parsed form of an OCI image reference,
implemented as a thin newtype around
[`oci_spec::distribution::Reference`]. It accepts
`host[:port]/name:tag`, `host[:port]/name@<digest>`, and the combined
`name:tag@<digest>` form on parse, and canonicalises digest references
to `host[:port]/name@<digest>` on Display (tag references keep the
`:` separator). `ImageRef` supersedes the previously re-exported
`ocipkg::ImageName` on every public surface:
[`LocalArtifact::open`](crate::artifact::LocalArtifact::open),
[`ArtifactDraft::new`](crate::artifact::ArtifactDraft::new),
the SQLite Local Registry helpers, and the CLI parse path all now take
[`ImageRef`](crate::artifact::ImageRef). The accessor shape is
`registry()` (the joined `host[:port]` form) plus `name()` /
`reference()` — the v2 split accessors `hostname` / `port` have
been removed since every internal consumer ended up rejoining them
back to `host[:port]` at the call site (callers that genuinely need
just the host portion, e.g. the localhost / 127.* heuristic in
`remote_transport::protocol_for`, parse the joined form inline).
Bare-namespace inputs without an explicit registry
(`library/ubuntu:20.04`, `alpine`) default to `docker.io` via the
standard Docker reference heuristic — the first segment is only
treated as a host when it contains `.` or `:` or equals `localhost`.
The `ommx::ocipkg` re-export is removed.

### v2 compat shim for Docker Hub shorthand

SDK v2 used `ocipkg`, which defaulted bare image names (`alpine`,
`ubuntu:20.04`) to the hostname `registry-1.docker.io`. v3 uses
`oci_spec::distribution::Reference`, whose canonical Docker Hub
hostname is `docker.io` (with `library/` automatically prefixed for
single-segment names). Without intervention, the same image would
land under two distinct SQLite Local Registry keys depending on
which side of the v2 → v3 boundary produced the string:
`load("alpine")` → `docker.io/library/alpine`, but a v2 cache
annotated as `registry-1.docker.io/alpine:latest` →
`registry-1.docker.io/alpine`. [`ImageRef::parse`](crate::artifact::ImageRef::parse)
rewrites the `"registry-1.docker.io/"` prefix to `"docker.io/"` before
delegating to `oci_spec`, collapsing every spelling of a Docker Hub
image onto the same canonical key. The rewrite requires the trailing
slash so adjacent hostnames like `registry-1.docker.io.example/...`
are left alone. `ocipkg`'s legacy `name:algorithm:hex` digest
spelling is *not* preserved — `oci_spec` rejects it — so digest-pinned
v2 annotations must already use the OCI-standard `name@<digest>`
form to round-trip (which is what `ocipkg`'s archive writer emitted
for digest refs in practice).

### `as_path` / `from_path` moved to the legacy module

`ocipkg::ImageName` exposed `as_path()` / `from_path()` for the v2
disk-cache local registry layout (`<root>/<image_name>/<tag>/`). The
v3 SQLite Local Registry stores blobs content-addressed and refs in
SQLite, so per-image directory paths are no longer a v3 concept.
The path-shape helpers have moved off `ImageRef` and are now internal to
the Local Registry implementation; the public v2-path compatibility entry point is
[`LocalRegistry::legacy_ref_path_in`](crate::artifact::local_registry::LocalRegistry::legacy_ref_path_in),
used by `ommx import-legacy` and the CLI's v2-only migration hint.
The previously public `get_image_dir` /
`image_dir` functions and the `ommx image-dir` CLI subcommand are
removed — their return value no longer corresponded to any v3
storage location.

### Accessor behaviour notes

- `reference()` returns the digest when both a tag and a digest are
  set (the OCI `name:tag@<digest>` combined form). OMMX has no code
  path that produces the combined form, so the tag-drop is theoretical
  for SDK-internal use; external callers who manually pass the combined
  form should be aware that round-tripping `ImageRef` through
  `(repository_key, reference)` keeps only the digest.

## Other notable changes

- `ommx-derive` introduces `#[derive(LogicalMemoryProfile)]` for structural
  memory profiling
  ([#800](https://github.com/Jij-Inc/ommx/pull/800)).
- `ommx::doc` is now the entry point on docs.rs for long-form prose
  (this page, the [migration guide](crate::doc::migration_guide), and the
  [tutorial](crate::doc::tutorial)).
