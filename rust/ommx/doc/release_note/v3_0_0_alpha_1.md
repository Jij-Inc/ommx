# 3.0.0-alpha.1

*Draft — not yet released.*

The 3.0.0 line is a major revision of the Rust SDK that rebuilds the core domain
types around a **lifecycle-stage type parameter** on constraints and collapses
the public error surface to a single type. It also finishes the long-running
migration away from the proto-generated `v1_ext` helpers: domain types
(`Instance`, `Constraint`, `DecisionVariable`, …) are now the primary API, and
`v1::*` is reserved for wire-format interop.

See the [migration guide](crate::doc::migration_guide) for the detailed
v2 → v3 upgrade path. This page is a topic-oriented summary of what changed and
why.

## Constraint lifecycle as a type parameter

`Constraint` is now generic over a [`Stage`](crate::Stage) marker:

```rust,ignore
pub struct Constraint<S: Stage<Self> = Created> {
    pub equality: Equality,
    pub metadata: ConstraintMetadata,
    pub stage: S::Data,
}
```

with three inhabited stages — `Created`, `Evaluated`, and `Sampled` — and
stage-specific data (`function`, `evaluated_value`/`feasible`,
`evaluated_values`/`feasible`). The constraint's `ConstraintID` is held
by the enclosing `BTreeMap` key rather than stored on the struct itself,
so standalone constraints are identity-less until inserted into a
collection.

The same pattern applies uniformly to
[`IndicatorConstraint`](crate::IndicatorConstraint),
[`OneHotConstraint`](crate::OneHotConstraint), and
[`Sos1Constraint`](crate::Sos1Constraint), all of which are now first-class
constraint types registered through the
[`ConstraintType`](crate::ConstraintType) trait (a defunctionalization of the
`Stage → Type` mapping since Rust lacks higher-kinded types).

Removed constraints no longer have a `Removed` stage. They are stored as
`(Constraint<Created>, RemovedReason)` tuples at the collection level — three
generic wrappers ([`ConstraintCollection`](crate::ConstraintCollection),
[`EvaluatedCollection`](crate::EvaluatedCollection),
[`SampledCollection`](crate::SampledCollection)) handle every constraint type
uniformly.

## Unified error surface

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

## Tracing-first observability

Internal logging has moved off `log` to `tracing`, and span coverage has been
broadened across parsing, evaluation, substitution, and solver adapter
entry points. Subscribers (including `tracing-opentelemetry`) pick up
structured fields and span context directly from the crate — no ad-hoc context
stacking via `anyhow::Error::context(...)` is needed.

## Domain types replace `v1_ext`

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

- Unsupported constraint types are now automatically converted at the
  `SolverAdapter` boundary (Big-M for indicator / SOS1, equality for one-hot).
- The `id` field has been removed from concrete constraint structs; the
  `BTreeMap<ID, _>` key is the sole source of truth.
- `ommx-derive` introduces `#[derive(LogicalMemoryProfile)]` for structural
  memory profiling.
- `ommx::doc` is now the entry point on docs.rs for long-form prose
  (this page, the [migration guide](crate::doc::migration_guide), and the
  [tutorial](crate::doc::tutorial)).
