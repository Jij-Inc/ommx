---
name: python-sdk-review
description: Use when reviewing OMMX Python SDK changes, PyO3 bindings under python/ommx/src, public Python API shape, pyo3-stub-gen output, generated stubs, Python-side wrappers, or tests for Python SDK behavior.
---

# Python SDK Review

Use this skill to review OMMX Python SDK changes from the binding boundary
outward: Rust/PyO3 source first, generated Python surface second, runtime tests
third. This is not a Sphinx documentation review; use
`.agents/skills/sphinx-docs-review/SKILL.md` when the main question is Sphinx,
AutoAPI, toctrees, or rendered documentation.

## Review Order

1. Identify the public Python operation.
   - Name the user-facing class, method, function, property, or magic method.
   - Check whether it is a stable SDK surface, an internal helper, or an
     adapter-facing protocol.
   - For domain operations, use `.agents/skills/domain-responsibility-review/SKILL.md`
     first and keep the Python API aligned with the owner boundary.
   - If the operation propagates Rust SDK failures or the diff touches
     `OmmxPyResult`, `python/ommx/src/error.rs`, signal mappings, or binding-local
     `map_err`, use `.agents/skills/rust-error-handling/SKILL.md` before choosing
     a Python exception class.

2. Review PyO3 source before generated stubs.
   - Start from `python/ommx/src/*.rs`, not from
     `python/ommx/ommx/_ommx_rust/__init__.pyi`.
   - For new exports, check `python/ommx/src/lib.rs`,
     `pyo3_stub_gen::reexport_module_members!`, and top-level Python package
     re-exports.
   - Treat generated stubs as evidence that the Rust binding source is modeled
     correctly, not as the place to hide an imprecise binding type.

3. Check input typing at the extraction boundary.
   - Prefer dedicated Rust input wrapper types with `FromPyObject` and
     `PyStubType` for public polymorphic inputs.
   - Use type aliases such as `ToFunction`, `ToState`, or `ToSamples` when
     users are expected to pass several supported Python shapes.
   - Avoid accepting `PyAny` in public methods unless the input is genuinely
     arbitrary Python data, adapter-owned, JSON-like, or intentionally opaque.

4. Check output typing at the conversion boundary.
   - Prefer a small Rust output wrapper that implements `IntoPyObject` and
     `PyStubType` when a method returns a fixed Python object shape.
   - Do not use `Bound<PyAny>` plus `#[gen_stub(override_return_type(...))]`
     just to make the generated `.pyi` look correct.
   - Keep `PyAny` returns for genuinely dynamic Python values, such as arbitrary
     attachments, adapter-owned reports, JSON payloads, trace/context-manager
     objects, or APIs whose concrete Python type is intentionally not owned by
     OMMX.
   - If the runtime object is a non-generated Python class or dataclass, the
     Rust wrapper may still construct that Python object in `IntoPyObject`; the
     public PyO3 method should return the wrapper type rather than raw `PyAny`.

5. Check runtime behavior and stub behavior together.
   - Run `task python:stubgen` after PyO3 public API, docstring, or
     `PyStubType` changes.
   - Inspect the generated `.pyi` for the affected items and confirm imports,
     optional defaults, overloads, and type aliases match the intended API.
   - Add or update focused Python tests for runtime behavior, including
     `isinstance` checks when the returned Python class matters.
   - For user-visible text/HTML/table output, prefer snapshot tests over
     piecemeal string fragments.

## Recurring Review Checks

- A correct-looking generated stub is not enough. Verify whether the Rust
  signature itself expresses the API shape through `FromPyObject`,
  `IntoPyObject`, `PyStubType`, `#[pyclass]`, or a type alias.
- A public method returning `Py<PyAny>`, `Bound<PyAny>`, or accepting `&PyAny`
  needs a reason. If the object shape is known, ask for a typed wrapper.
- `#[gen_stub(override_return_type(...))]` is acceptable for exceptional
  generated-stub limitations, not as the normal way to type a fixed return.
- Arithmetic and magic methods often need `#[gen_stub(skip)]` or specialized
  stub handling; review their runtime return classes and type hints together.
- When a Python API exposes Rust-owned data plus Python convenience behavior,
  check both ownership directions: extraction into Rust and conversion back to
  Python.
- Keep generated artifacts synchronized with the source change:
  `python/ommx/ommx/_ommx_rust/__init__.pyi`,
  generated package `__init__.py` files, and `docs/api/api_reference.json` when
  stubgen updates it.
- If the diff touches rendered docs, AutoAPI warnings, or Sphinx wiring, switch
  to `.agents/skills/sphinx-docs-review/SKILL.md` for that part.

## Validation Commands

Use the narrowest commands that match the diff. Python commands must run
through the project environment.

- After PyO3 public API or `PyStubType` changes:
  ```
  task python:stubgen
  cargo check -p _ommx_rust
  ```
- For Python SDK behavior:
  ```
  uv run pytest python/ommx-tests/tests/<target>.py
  ```
- For generated-file hygiene:
  ```
  git diff --check
  git status -sb
  ```

## Findings

Lead findings with user-visible API or binding-boundary risk:

- Public stub type differs from runtime behavior.
- Public PyO3 signature uses opaque `PyAny` where a typed wrapper should own
  extraction or conversion.
- Generated stubs were not regenerated after a public API change.
- Top-level Python re-exports are missing or stale.
- Python behavior changed without focused runtime tests.
- Output behavior changed without snapshots when the output is the feature.
