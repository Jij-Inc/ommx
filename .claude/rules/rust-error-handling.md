---
paths:
  - "rust/ommx/**/*.rs"
---

# Rust SDK Error Handling Policy

The `ommx` crate uses a single unified error surface. New code should follow the same shape.

## Rules

- **Public APIs return `ommx::Result<T>`** — an alias for `anyhow::Result<T>`. Do not introduce `anyhow::Result<T>` in public signatures; use `ommx::Result<T>` (and `ommx::Error`) so the crate name stays on the API surface.
- **Do not introduce new typed error enums for purely diagnostic purposes.** Previous enums that only existed to give each failure mode a variant name (`InstanceError`, `MpsParseError`, `StateValidationError`, `LogEncodingError`, …) were deleted because downstream consumers never matched on discriminants. Failure sites should use `crate::bail!` / `crate::error!` with a clear `Display` message.
- **Signal types** (`InfeasibleDetected`, `CoefficientError`, `BoundError`, `AtolError`, `DecisionVariableError`, `SubstitutionError`, `SolutionError`, `SampleSetError`) stay typed because callers recover them via `err.downcast_ref::<T>()`. If you add a new recoverable failure mode, consider adding it to this list rather than returning `Err(anyhow::anyhow!(…))`.
- **Narrow-domain parsers may keep a small structured error type** that carries position / path information and is converted to `ommx::Error` at the domain boundary. The typed form lets callers report the failure programmatically (e.g. editor squiggles); the boundary conversion emits a `tracing::error!` event with the same information. The canonical examples are:
  - `ommx::ParseError` (proto-tree breadcrumbs — `Vec<ParseContext>`).
  - `ommx::QplibParseError` (1-based line number + rendered message, with `tracing::error!` emitted on construction).

  Keep the scope tight: one structured type per small domain, only when there is concrete programmatic value (a position, a path) beyond the rendered message. Don't reintroduce large discriminant-enum error types in the name of "structure".
- **Diagnostic context flows through `tracing`, not through `anyhow::Error::context(...)`.** The fail-site macros below already emit a `tracing::error!` event alongside the `anyhow::Error` they build, so there is no need to stack context via `anyhow::Context`. When propagating a caught error, `.inspect_err(|e| tracing::error!(…))` + `?` keeps the original error chain intact for downcast; wrapping through `.context(...)` is reserved for the narrow-domain cases above.
- **`Option<T>` for key lookups**, not `Result<T, UnknownXXXError>`. Missing keys are not errors.

## Fail-site macros

Use the fail-site macros at every internal `Err(...)` site. They emit a `tracing::error!` event alongside producing an `anyhow::Error` from the same format string.

```rust
// Plain message — the format string is shared between the tracing event
// and the error's Display output.
crate::bail!("plain message with {interpolated} value");

// Structured tracing fields via `{ field = value, … }`. The `?foo` / `%foo`
// prefixes forward to tracing's Debug / Display capture.
crate::bail!(
    { section, size, id = ?id },
    "invalid field size ({size}) in section '{section}'",
);

// Signal expression — no tracing event, since callers typically recover
// by downcast rather than observe as diagnostics.
crate::bail!(InfeasibleDetected);

// Short-circuit if cond is false.
crate::ensure!(cond, "message if cond is false");

// Inline (for `.ok_or_else(|| …)`).
let err = crate::error!("inline construction");
```
