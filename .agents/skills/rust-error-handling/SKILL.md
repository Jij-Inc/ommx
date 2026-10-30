---
name: rust-error-handling
description: Use when designing, implementing, or reviewing OMMX Rust SDK fallible APIs, public error or signal types, anyhow error chains, `ommx::Result`, PyO3 `OmmxPyResult` conversions, Python exception mappings, or binding-local `map_err` and `?` propagation.
---

# Rust Error Handling

Use `.agents/skills/domain-responsibility-review/SKILL.md` first to establish
the domain owner. Read `rust/ommx/doc/tutorial/error_handling.md` before changing
or reviewing a public Rust error surface. Treat every public signal and Python
exception classification as an SDK contract.

## Mandatory Decision Order

1. Name the failing domain operation and its owner.
2. State the caller's concrete recovery action: what will the caller inspect,
   change, retry, skip, or select differently?
3. Verify that recovery is an intentional, stable contract of that public
   operation, not merely a hypothetical reaction available to one internal or
   application-specific consumer.
4. Choose the narrowest error surface that supports that contract.

If no stable, intentional recovery contract can be named, do not add a public
signal merely to improve diagnostics or select a Python exception class. If an
operation is already documented as producing an existing signal for that
condition, treat an untyped path as a contract bug rather than inventing a new
parallel signal.

| Condition | Error surface |
| --- | --- |
| No stable caller recovery path | `ommx::Result<T>` with an ordinary `ommx::Error` |
| One recoverable condition among broader failures | Public OMMX-owned signal preserved in the `ommx::Error` chain |
| A narrow API has one signal failure that callers commonly match directly | A typed `Result<T, Signal>` may be justified |
| Python owns the failure semantics | Explicit `PyErr` classification at the binding site |

## Rust SDK Policy

- Keep `ommx::Result<T>` as the default public fallible return type. Do not
  replace it with a closed SDK-wide error enum.
- Curate public signal types for stable programmatic recovery. Preserve them in
  the `anyhow::Error` chain so callers can use `is` or `downcast_ref`.
- Keep each signal with the narrowest domain owner that has enough information
  to define it. Reuse an existing owner signal before introducing another type.
- Do not create an operation-wide umbrella type that combines failures with
  different owners or recovery actions. A domain-owned enum remains appropriate
  when its variants form one stable matching contract.
- Do not expose `prost`, `serde_json`, OCI, registry-client, or other dependency
  errors as the OMMX consumer contract. Normalize them at the OMMX boundary only
  when callers need a stable OMMX-owned signal.
- Preserve sources and diagnostic context while their recovery semantics remain
  valid at the outer owner boundary. Do not replace such a signal with a
  rendered string or duplicate its message in a wrapper.
- Review nested signals independently from their outer owner. If the inner
  signal still truthfully describes the cause and the outer owner has its own
  stable recovery contract, preserve the inner source beneath that outer
  OMMX-owned signal and map the outer signal first. Rust callers may then
  intentionally downcast to either layer while Python follows outer ownership.
- Do not leave an inner signal discoverable when its recovery contract becomes
  false or misleading after an ownership transition, such as caller-input
  validation reused to validate internally derived state. Convert that cause
  to non-signal diagnostic context, then emit an outer OMMX-owned signal if one
  is justified or an ordinary `ommx::Error` otherwise. Retain the relevant
  cause and values as structured tracing fields and regression-test the outer
  mapping or fallback semantics.
- Expose the payload a caller needs for recovery through public fields or
  accessors. Document its meaning and use `#[non_exhaustive]` when callers should
  inspect but not exhaustively construct the signal.
- Keep failures without a stable recovery path as ordinary `ommx::Error`
  values, with structured tracing at the fail site.

For every new public signal, also use
`.agents/skills/rust-module-boundary/SKILL.md` to justify its module owner,
visibility, payload, and public API commitment.

## PyO3 Boundary Policy

Classify ownership before choosing a Python exception.

### Rust SDK-owned failures

- Return `OmmxPyResult<T>` from exposed bindings that can propagate Rust-owned
  failures.
- Declare each OMMX-owned signal mapping once in the shared mapping table in
  `python/ommx/src/error.rs`. Generate both the direct `From<Signal>` conversion
  and fallback dispatch for a signal already erased into `ommx::Error`.
- Use normal `?` propagation at binding call sites. Do not add per-call
  `map_err` closures for Rust SDK-owned failures.
- Preserve outer domain ownership when an error chain contains nested signals;
  keep fallback classification order explicit where it affects the contract.
- Map unclassified Rust SDK failures to `RuntimeError`.
- Do not enable or depend on PyO3's blanket `anyhow` conversion, add a generic
  conversion that absorbs arbitrary errors, or classify by rendered message.

### Python-owned failures

Keep Python semantics explicit at the binding boundary. Do not force these
through the Rust SDK signal mapper:

- Python argument extraction or protocol violations (`TypeError`)
- Python integer conversion overflow (`OverflowError`)
- Mapping-like lookup behavior owned by the Python API (`KeyError`)
- Python object or JSON input validation (`ValueError`)
- Exceptions from imports, callbacks, adapters, pandas, codecs, or other Python
  libraries; preserve the original `PyErr`

Malformed payloads received by the private cross-extension bridge are
binding-internal Rust failures and fall back to `RuntimeError`; they are not
Python call-signature `TypeError` cases unless an original `PyErr` exists.

The existence of a local `map_err` is not itself a defect. First determine
whether Rust SDK or Python owns the failure.

## Review Workflow

1. Inventory the changed failure paths.
   - List every public Rust entry point and exposed PyO3 method affected.
   - Search sibling paths for the same failure so equivalent operations do not
     acquire different contracts.
2. Classify every failure.
   - Name its owner and caller recovery action.
   - Search for an existing signal before adding one.
   - Separate caller input, persisted corruption, dependency failure, and
     internal invariant failure.
3. Verify propagation.
   - Confirm the signal survives direct typed conversion and prior erasure into
     `ommx::Error`.
   - Confirm nested mapped signals do not override a more specific outer owner;
     declaration order is part of the Python contract.
   - Confirm Python-owned `PyErr` values pass through unchanged.
   - Confirm ordinary SDK failures reach the `RuntimeError` fallback.
4. Verify mutation and payload behavior.
   - Validate all inputs before mutation or prove rollback/atomicity.
   - Test every recovery-relevant payload field, including value direction and
     the complete set of missing or unknown IDs.
5. Verify user-facing consistency.
   - Test equivalent failures across entry points against the same Python
     exception class.
   - Update Rustdoc, Python docstrings, migration notes, release notes, stubs,
     and API metadata when the public contract changes.

## Evidence and Commands

Require both enforcing code and a regression test for every claimed contract.
The shared mapping mechanism must have unit coverage for direct and erased
conversion, ordinary fallback, and unchanged `PyErr` pass-through. A macro does
not need the same mechanism test repeated for every entry, but every newly
mapped signal needs a focused Python integration test through its real public
operation. Add both direct and erased tests for a particular signal only when
it can reach the boundary through distinct conversion paths not already proved
by the shared mechanism. When mapped outer and inner signals can coexist in one
chain, test that declaration order selects the outer Python contract. When an
ownership transition intentionally removes an obsolete inner signal, test both
that Rust can no longer downcast to it and that Python selects the intended
outer or fallback exception.

Use focused commands that match the touched surface:

```bash
rg -n "OmmxPyResult|define_ommx_error_mappings|map_err|new_err|downcast_ref" \
  rust/ommx/src python/ommx/src
cargo test -p ommx affected_module --lib
env PYO3_PYTHON="$PWD/.venv/bin/python3" cargo test -p _ommx_rust error::tests --lib
uv run --no-sync pytest -q python/ommx-tests/tests/test_affected_behavior.py
cargo tree -p _ommx_rust -e features -i pyo3
```

Run `task python:sync` first when the project `.venv` is absent.
Run `task python:stubgen` after a public PyO3 signature or docstring change.
Run `cargo fmt --all -- --check` and `git diff --check` before handoff.

## Checklist

- What will a caller do differently after identifying this failure, and is that
  recovery an intentional contract of this operation?
- Which domain owns the signal and its payload?
- Can an existing signal express the same recovery contract?
- Does the public API remain on `ommx::Result` unless a narrow typed result is
  justified?
- Does the error chain preserve only signals whose recovery semantics remain
  valid at the outer owner, plus the useful source and diagnostic context?
- Are dependency types and rendered messages absent from the public contract?
- Do direct and erased Rust errors share one declarative Python mapping?
- Are Python-owned exceptions kept outside the SDK mapper?
- Are all recovery payload fields, fallback behavior, and mutation atomicity
  protected by tests?
