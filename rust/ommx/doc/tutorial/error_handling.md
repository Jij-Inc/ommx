# Error Handling

Most public fallible APIs return [`Result<T>`](crate::Result) (alias for
`std::result::Result<T, Error>`). [`Error`](crate::Error) is a re-export of
`anyhow::Error`, so downstream crates can propagate with `?` without
taking an `anyhow` dependency themselves. Diagnostic context is emitted
via the [`tracing`](https://docs.rs/tracing) crate at each failure site rather than carried in
typed enum variants — subscribers pick it up via span context and
structured fields.

A small number of APIs return a typed error directly rather than
`ommx::Result<T>` — specifically [`DecisionVariable::with_bound`](crate::DecisionVariable::with_bound),
the [`SampleSet::best_feasible`](crate::SampleSet::best_feasible) family,
[`Sampled::append`](crate::Sampled::append), and a few builders — because their
single failure mode is already a
**signal type** and the typed return surfaces that at the call site.
Every such typed error implements [`std::error::Error`], so `?` still
lifts it into `ommx::Result<T>` at a domain boundary; the distinction
only matters when a caller wants to `match` on the failure without
first going through `err.downcast_ref::<T>()`.

A curated set of **signal types** remain `pub` for callers that need to
recover a particular failure — either by downcast (when the failure
arrived as `ommx::Error`) or by direct pattern matching (when an API
returns the typed error directly):

- [`InfeasibleDetected`](crate::InfeasibleDetected) — produced by [`Propagate`](crate::Propagate) when a constraint
  becomes infeasible after substitution.
- [`CoefficientError`](crate::CoefficientError), [`BoundError`](crate::BoundError), [`AtolError`](crate::AtolError),
  [`InvalidPenaltyWeight`](crate::InvalidPenaltyWeight) — numeric-domain validation failures.
- [`FunctionEvaluationError`](crate::FunctionEvaluationError) — identifies
  undefined division, a zero-classified value raised to a negative integer
  power, and non-finite function results. At evaluation time, the caller's
  [`ATol`](crate::ATol) classifies a value as zero when `abs(value) <= atol`,
  including the boundary. A caller can change the state, expression, or
  evaluation tolerance before retrying.
- [`FixedPenaltyWeightIDMismatch`](crate::FixedPenaltyWeightIDMismatch) —
  identifies the missing and unexpected active constraint IDs in a caller-owned
  fixed-penalty weight map, so the caller can correct the keys and retry the
  atomic operation on the unchanged [`Instance`](crate::Instance).
- [`Sos1BigMPromotionBatchRejected`](crate::Sos1BigMPromotionBatchRejected) —
  owns every rejected zero-based input index and its error chain when a caller
  requests fully-valid SOS1 Big-M batch application. The caller can repair or
  remove those requests and retry against the unchanged
  [`Instance`](crate::Instance).
- [`DecisionVariableError`](crate::DecisionVariableError), [`SubstitutionError`](crate::SubstitutionError), [`SolutionError`](crate::SolutionError),
  [`SampleSetError`](crate::SampleSetError) — domain-specific structured errors consumed by
  in-crate tests and downstream code that wants to react programmatically.
- [`DuplicatedSampleIDError`](crate::DuplicatedSampleIDError) — identifies a
  sample ID already present in a [`Sampled`](crate::Sampled) collection or
  repeated in one append input, so the caller can choose another ID and retry
  the atomic append.
- [`SamplesParametersError`](crate::random::SamplesParametersError) —
  identifies invalid relations among random-sample counts and the inclusive ID
  range, so the caller can correct the requested parameters before retrying.
- [`ParameterIDCollision`](crate::ParameterIDCollision) — identifies a
  decision-variable ID already owned by a parameter, so the caller can choose
  another ID before retrying construction or insertion.
- [`ContentFactorError`](crate::ContentFactorError) — identifies coefficients
  that cannot be converted to a bounded rational multiplier, so the caller can
  change the coefficients or choose another normalization operation.
- [`OneHotConstraintError`](crate::OneHotConstraintError) and
  [`Sos1ConstraintError`](crate::Sos1ConstraintError) — identify empty
  structural constraints, so the caller can supply a non-empty variable set.
- [`MissingStateEntries`](crate::MissingStateEntries) and
  [`UnknownStateEntries`](crate::UnknownStateEntries) — state-shape signals for
  callers that add or remove entries before retrying evaluation.
- [`InconsistentDependentValue`](crate::InconsistentDependentValue) and
  [`UnverifiableDependentAssertion`](crate::UnverifiableDependentAssertion) —
  dependent-variable assertion signals for callers that correct, defer, or
  complete an assertion before retrying partial evaluation.
- [`ImageRefParseError`](crate::artifact::ImageRefParseError) and
  [`InvalidLocalRegistryImageRef`](crate::artifact::local_registry::InvalidLocalRegistryImageRef) —
  distinguish invalid image-reference input from an invalid name/reference pair
  already persisted in the Local Registry.
- [`AttachmentNotFound`](crate::experiment::AttachmentNotFound) — identifies
  an absent Attachment name in an Experiment or Run namespace.
- [`LogEncodingUnavailable`](crate::LogEncodingUnavailable) and
  [`ExactIntegerSlackUnavailable`](crate::ExactIntegerSlackUnavailable) — identify
  the narrow cases where an exact encoding operation is unavailable and a
  caller may explicitly choose another mathematical operation or postcondition.
  Contract, allocation, substitution, and arithmetic failures are not folded
  into these signals.
- [`PreparationTargetNotReached`](crate::PreparationTargetNotReached) — reports
  that all configured Preparation phases completed without establishing the
  target [`InstanceClass`](crate::InstanceClass) membership. Callers can inspect
  its typed membership report before adding phases, revising the target class,
  or reporting the remaining mismatches.

Evaluation does not define an operation-wide umbrella error type.
Caller-provided decision-variable validation reuses
[`DecisionVariableError`](crate::DecisionVariableError), while
[`FunctionEvaluationError`](crate::FunctionEvaluationError) covers the
function-owned undefined-domain and non-finite-result conditions above.
Failures without a stable caller recovery path remain ordinary
[`Error`](crate::Error) values.

Direct function and polynomial partial evaluation retain
[`CoefficientError`](crate::CoefficientError), because the caller can change
the supplied state and retry. If the same arithmetic fails while an
[`Instance`](crate::Instance) normalizes an Instance-owned dependency or a
removed constraint against stored dependencies and fixed values, that signal
no longer describes caller input and is converted to an ordinary
[`Error`](crate::Error) with structured tracing context.

The same ownership transition applies to
[`FunctionEvaluationError`](crate::FunctionEvaluationError): direct function
evaluation and evaluation of caller-owned
[`AcyclicAssignments`](crate::AcyclicAssignments) retain the signal, while an
internally derived, Instance-owned dependent-variable value converts it to
ordinary owner-contextualized [`Error`](crate::Error).

Recover them with [`Error::downcast_ref`](crate::Error::downcast_ref) / [`Error::is`](crate::Error::is):

```ignore
match instance.propagate(&state, atol) {
    Err(e) if e.is::<ommx::InfeasibleDetected>() => { /* handle */ }
    Err(e) => return Err(e),
    Ok(outcome) => { /* ... */ }
}
```

For example, a caller that does not require the inequality to become an
equality can explicitly select the inequality-preserving Integer slack
operation after the exact-operation signal, while continuing to propagate
unrelated failures:

```ignore
match instance.convert_inequality_to_equality_with_integer_slack(id, 32, atol) {
    Err(e) if e.is::<ommx::ExactIntegerSlackUnavailable>() => {
        // This operation keeps the relation as an inequality. It is not an
        // approximate representation of the original feasible set.
        instance.add_integer_slack_to_inequality(id, 32, atol)?;
    }
    Err(e) => return Err(e),
    Ok(()) => {}
}
```

If exact integer-slack conversion cannot normalize the coefficients, the same
error chain retains both the outer
[`ExactIntegerSlackUnavailable`](crate::ExactIntegerSlackUnavailable) signal
and its inner [`ContentFactorError`](crate::ContentFactorError). Callers can
therefore choose an inequality-preserving transformation from the outer
operation signal or change the coefficients based on the narrower cause.

Protobuf wire decoding and the [`Parse`](crate::Parse) trait share the
[`ParseError`](crate::ParseError) signal. Public byte decoders preserve wire
failures as `ParseError` in their [`Result<T>`](crate::Result) error chain,
while semantic parsing adds structured
[`Vec<ParseContext>`](crate::parse::ParseContext) breadcrumbs with useful
proto-tree metadata. [`ParseError`](crate::ParseError) implements
[`std::error::Error`], so callers can downcast the SDK error or propagate it
with `?`.

Semantic parsing keeps `ParseError` as the outer owner while retaining a
narrower validation signal in its standard source chain. This includes
[`ParameterIDCollision`](crate::ParameterIDCollision) for v1 and v2
ParametricInstance namespace collisions, and
[`OneHotConstraintError`](crate::OneHotConstraintError) or
[`Sos1ConstraintError`](crate::Sos1ConstraintError) for v2 special-constraint
validation. This preserves the validation cause without changing the
Python-visible parse contract.

[`ParseError`](crate::ParseError) exposes its immediate cause through
[`std::error::Error::source`]. Recoverable semantic failures can therefore be
downcast directly from that source to their domain signal without matching a
`RawParseError` wrapper. [`RawParseError`](crate::RawParseError) is
reserved for generic protobuf-boundary failures such as missing fields,
unknown enums, reserved annotation keys, and decode errors. Dedicated
message-specific parse signals such as
[`QuadraticParseError`](crate::QuadraticParseError) are also exposed directly.
Semantic failures without a caller recovery path remain ordinary errors and
still retain the `ParseError` breadcrumbs.

```ignore
let parse_error = error.downcast_ref::<ommx::ParseError>().unwrap();
let cause = std::error::Error::source(parse_error).unwrap();

if let Some(solution_error) = cause.downcast_ref::<ommx::SolutionError>() {
    // Inspect the Solution-owned signal and recover if appropriate.
}
```

## Fail-site macros

[`bail!`](crate::bail), [`error!`](crate::error!), and [`ensure!`](crate::ensure) fuse a `tracing::error!` event
with an [`Error`](crate::Error) built from the same format string:

```ignore
// Plain message
ommx::bail!("invalid OBJSENSE: {s}");

// Structured tracing fields via `{ field = value, … }`
ommx::bail!(
    { section, size },
    "invalid field size ({size}) in MPS section '{section}'",
);

// Signal expression — no tracing event, since callers recover it
ommx::bail!(InfeasibleDetected);
```
