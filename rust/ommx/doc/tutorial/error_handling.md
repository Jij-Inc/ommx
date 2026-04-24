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
and a few builders — because their single failure mode is already a
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
- [`CoefficientError`](crate::CoefficientError), [`BoundError`](crate::BoundError), [`AtolError`](crate::AtolError) — numeric-domain
  validation failures.
- [`DecisionVariableError`](crate::DecisionVariableError), [`SubstitutionError`](crate::SubstitutionError), [`SolutionError`](crate::SolutionError),
  [`SampleSetError`](crate::SampleSetError) — domain-specific structured errors consumed by
  in-crate tests and downstream code that wants to react programmatically.

Recover them with [`Error::downcast_ref`](crate::Error::downcast_ref) / [`Error::is`](crate::Error::is):

```ignore
match instance.propagate(&state, atol) {
    Err(e) if e.is::<ommx::InfeasibleDetected>() => { /* handle */ }
    Err(e) => return Err(e),
    Ok(outcome) => { /* ... */ }
}
```

The [`Parse`](crate::Parse) trait is an intentional exception. It keeps its own
[`ParseError`](crate::ParseError) type because the structured
[`Vec<ParseContext>`](crate::parse::ParseContext) breadcrumb carries useful
proto-tree metadata. [`ParseError`](crate::ParseError) implements [`std::error::Error`], so
it flows into [`Result<T>`](crate::Result) via `?` at the crate boundary.

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
