---
paths:
  - "python/ommx/src/**/*.rs"
---

# PyO3 Binding Development

## Architecture

- `python/ommx/src/` contains PyO3 bindings that wrap Rust types from `ommx::` crate
- The compiled extension module is `ommx._ommx_rust`
- Python users import from `ommx.v1` (auto-generated re-exports), never from `_ommx_rust` directly

## Adding a new type

1. Create `python/ommx/src/<name>.rs` wrapping the `ommx::` type
2. Add `mod <name>;` and `pub use <name>::*;` in `lib.rs`
3. Register with `m.add_class::<TypeName>()?;` in the `_ommx_rust` pymodule function in `lib.rs`
4. Add to `reexport_module_members!("ommx.v1" ...)` in `lib.rs` for `ommx.v1` re-export

## Required decorators

Classes and enums:

```rust
#[pyo3_stub_gen::derive::gen_stub_pyclass]    // struct
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum] // enum
#[pyclass]
pub struct TypeName(pub ommx::TypeName);
```

Methods:

```rust
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl TypeName { ... }
```

Functions:

```rust
#[pyo3_stub_gen::derive::gen_stub_pyfunction]
#[pyfunction]
pub fn func_name(...) { ... }
```

The `gen_stub_*` decorators must always be paired with the corresponding PyO3 decorator. Without them, stub generation will miss the type.

## Type conversion pattern

Every binding type needs bidirectional `From` impls between the PyO3 wrapper and the `ommx::` type:

```rust
impl From<ommx::TypeName> for TypeName { ... }
impl From<TypeName> for ommx::TypeName { ... }
```

## Generated files (do not edit manually)

`task python:stubgen` runs `cargo run --bin stub_gen` which invokes pyo3-stub-gen to generate:

- **`ommx/_ommx_rust/__init__.pyi`** — type stubs for the compiled extension module
- **`ommx/v1/__init__.py`** — re-exports from `_ommx_rust`, configured by `generate-init-py = ["ommx.v1", "ommx.artifact"]` in `pyproject.toml`
- **`ommx/artifact/__init__.py`** — re-exports for artifact types
- **`docs/api/`** — API reference docs (`.rst` files and `api_reference.json`), configured by `[tool.pyo3-stub-gen.doc-gen]` in `pyproject.toml`

## After making changes

- `task python:sync` — rebuild the extension module (runs `uv sync` which triggers maturin)
- `task python:stubgen` — regenerate all files listed above

## Free-threaded Python support

The module uses `#[pymodule(gil_used = false)]` for Python 3.13t compatibility. Avoid storing Python objects in global state.

## Stub-side type customization: define a Rust type, not `override_type`

`pyo3-stub-gen` exposes a per-argument `#[gen_stub(override_type(type_repr = "...", imports = (...)))]` attribute that lets you write a Python type expression directly into the generated stub. **Reserve this for one-shot cases only** — typically when a single argument needs a fully ad-hoc Python type that has no first-class meaning in the Rust codebase (e.g. a `**kwargs` HashMap rendered as `str`).

For any type that appears at multiple call sites — `Literal[...]`, union of accepted Python types, etc. — define a Rust type instead and put the stub representation on the type itself:

1. **`FromPyObject`** — runtime validation / conversion (`Borrowed<'_, 'py, PyAny>` → `Self`).
2. **`IntoPyObject`** (when the value can appear as a default or be returned) — so `pyo3-stub-gen` can render defaults via `fmt_py_obj`.
3. **`pyo3_stub_gen::PyStubType`** — owns the `type_input()` / `type_output()` `Literal[...]` / union representation and any required `imports`.

Existing examples:

- `ConstraintKind` in `python/ommx/src/pandas.rs` — string `Literal[...]` on the way in, normalised to a Rust `enum` before downstream `match` arms see it.
- `Function` / `ToFunction` in `python/ommx/src/function.rs` — multi-type union (`int | float | DecisionVariable | … | Function`) with `pyo3_stub_gen::type_alias!` for the Python-side alias.
- `Samples` / `ToSamples` in `python/ommx/src/samples.rs` — same pattern with extra `Mapping[int, ToState]` / `Iterable[ToState]` marker types.

Why: `override_type` duplicates the same `type_repr` / `imports` literal at every call site, which breaks both DRY (changing the literal means hunting through N annotations) and exhaustiveness (a Rust `enum` gets compile-time `match` checks; a `&str` argument validated ad-hoc does not). One Rust type, one set of impls, one source of truth.
