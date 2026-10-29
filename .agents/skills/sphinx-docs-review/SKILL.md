---
name: sphinx-docs-review
description: "Review OMMX documentation publication correctness: Sphinx/docutils build and rendering failures, unresolved references, generated API wiring, and consistency of PyO3 docstrings, exports, stubs, and API pages. Use for those technical concerns, not for Book prose structure, reader prerequisites, or learning-path design alone."
---

# Sphinx Docs Review

Check whether handwritten documentation and public API declarations are
faithfully exposed in generated references and rendered output. This is a
technical publication workflow, not a general Book-content review.

Select it for a build/render/link failure or a change to the documentation
generation pipeline. A file being under `docs/` is not sufficient by itself.
For example, a PyO3 docstring parse failure needs this workflow; reordering a
Tutorial's explanation of feasibility does not.

Reader prerequisites, concept order, cognitive load, and independent reading
paths belong to `book-reading-review`. Use both only when the request actually
includes both concerns; neither skill requires loading the other or running
its checks merely because documentation changed.

Treat generated files as evidence of the handwritten source/configuration, not
as the first review surface. Start from the owner of the documentation behavior,
then sample generated artifacts to confirm the behavior is reflected correctly.

## Review Order

1. Identify the technical publication surface affected.
   - Rendered Book source: the relevant files under `docs/en` or `docs/ja`
     when inspecting directives, API roles, toctrees, links, or rendering.
   - API reference: `docs/api`, `docs/conf_base.py`,
     `docs/api/pyo3_stub_gen_ext.py`, AutoAPI configuration, and generated API
     item pages.
   - Python binding docs/stubs: `python/ommx/src/*.rs`,
     `python/ommx/src/docstrings/`,
     `python/ommx/src/lib.rs`, `python/ommx/src/bin/stub_gen.rs`,
     `python/ommx/pyproject.toml`, `python/ommx/ommx/_ommx_rust/__init__.pyi`,
     and generated `__init__.py` files.

2. Review handwritten sources before generated artifacts.
   - For wiring or reference problems, check the relevant entrypoints/toctrees:
     `docs/api/index.rst`, `docs/conf_base.py`, `docs/en/index.md`,
     `docs/ja/index.md`.
   - For PyO3-backed API changes, check `pyo3_stub_gen::reexport_module_members!`
     in `python/ommx/src/lib.rs` and `[tool.pyo3-stub-gen]` in
     `python/ommx/pyproject.toml`.
   - For docstring changes, review the Rust/Python source docstrings and their
     included Markdown that feed generated stubs and `docs/api/api_reference.json`.

3. Verify generated artifacts for consistency, not prose quality.
   - For API/docstring generation changes, confirm the applicable generated
     files changed together:
     `python/ommx/ommx/_ommx_rust/__init__.pyi`,
     generated package `__init__.py` files, `docs/api/api_reference.json`,
     `docs/api/*.rst`, and `docs/api/_items/*.rst`.
   - Check for stale generated files that should have been deleted, especially
     old module/package names after public API moves:
     ```
     git ls-files docs/api docs/api/_items | rg 'old-name|ommx\.v1'
     rg -n '"module": "old-name"|"qualified_name": "old-name' docs/api/api_reference.json
     ```
   - Do not line-review large generated JSON diffs except to confirm module
     names, item names, and docstring content are sourced from the expected
     handwritten change.

4. Separate Python import namespace from protobuf/media namespace.
   - `ommx.v1` may be correct in protobuf wire formats, media types, annotation
     namespaces, and historical migration examples.
   - `ommx.v1` is suspect in Python SDK imports, generated API package names,
     examples showing the current API, adapter code, and new tests.

5. Review Sphinx/docutils failure modes explicitly.
   - A missing toctree entry can hide the next docstring parse warning; rebuild
     after each wiring fix.
   - Prefer reST field syntax such as `:param name:` for exported Python
     docstrings when AutoAPI/docutils parse stability matters.
   - Check for `toc.not_included`, unexpected indentation, block quotes missing
     blank lines, duplicate labels, and unresolved class/method references.

## Markdown Docstring Sources

Keep external Python binding docstrings in `python/ommx/src/docstrings/`, using
public Python module names as directories and the public object name, with its
original capitalization, as the filename. For example, `ommx.ConstraintCollection`
uses `ommx/ConstraintCollection.md`, and `ommx.experiment.Experiment` uses
`ommx/experiment/Experiment.md`. An external method docstring uses a filename such
as `ommx/Instance.evaluate.md`. Resolve names from public exports, not Rust type
names or the internal `ommx._ommx_rust` module.

Use Markdown for long explanations and keep short descriptions inline in `///`.
In Markdown sources, wrap Python REPL examples in `pycon` code fences so Markdown
and clippy do not interpret `>>>` as block quotes. Leave a blank line before the
closing fence so Python doctest does not treat it as expected output.
Each documented API has one entry source, normally one complete Markdown file.
Prefer linking to the API that owns a shared explanation. If splitting an entry
becomes useful, prefix its fragments with the owning API name; choose separators
and suffixes when a concrete split is needed rather than introducing speculative
fragment conventions now.

Feed the same complete text to runtime docstrings and stub/reference metadata.
If fragments are introduced, compose them before those consumers; Sphinx-only
includes would leave `help()` and runtime doctests with incomplete documentation.
When moving a source, update its includes, direct test readers, and any manifest
source paths together.

Use `#[doc = include_str!("docstrings/ommx/PublicClass.md")]` with the normal
`gen_stub_pyclass` or `gen_stub_pymethods` macros. The workspace requires
`pyo3-stub-gen` 0.23.2 or later, which preserves `include_str!` and `concat!`
expressions in generated metadata. For classes with a custom `PyStubType`, use
`gen_stub_pyclass(skip_stub_type)` and retain their runtime type registration;
preserve any explicit stub module. Manual registration may still be needed for
metadata the derive cannot express, such as virtual Mapping bases. Compare the
complete Markdown with runtime docstrings, generated stubs, and API metadata.

## Validation Commands

Use the narrowest commands that match the diff. Python commands must run through
the project environment.

Book code is explanatory pseudocode: review its meaning, without requiring or
claiming execution. Working API examples belong in Reference docstrings checked
by `python/ommx-tests/tests/test_doctests.py`; runnable end-to-end workflows belong
in standalone Examples and their existing tests. This separation does not
authorize disabling, weakening, or adding skips to existing Notebook tests.
Do not add Notebook reruns or enable execution for a Book review. An HTML build
verifies publication, not code execution.

- After PyO3 public API or docstring changes:
  ```
  task python:stubgen
  ```
- When API examples are added, changed, or moved from the Book, rebuild the
  current SDK before checking the affected Reference doctests. Stub generation
  alone does not refresh the imported extension's runtime docstrings. From the
  repository root, select the affected docstring owners with `-k`; for example,
  for changes to `Instance`:
  ```
  task python:sync
  uv run --no-sync pytest -vv python/ommx-tests/tests/test_doctests.py -k 'ommx.Instance'
  ```
  Use `task python:ommx:pytest-doctest` when the full doctest suite is appropriate.
  Report the selected doctests separately from publication checks; neither a
  generated example nor a successful HTML build establishes execution.
- Build docs as warnings-as-errors:
  ```
  uv run sphinx-build -b html -W --keep-going docs/en docs/en/_build/html
  uv run sphinx-build -b html -W --keep-going docs/ja docs/ja/_build/html
  ```
- Check generated-file hygiene:
  ```
  git diff --check
  git status -sb
  git ls-files docs/api docs/api/_items | rg 'ommx\.v1|old-package-name'
  ```
- For import-surface migrations, search both runtime and docs:
  ```
  rg -n 'from ommx\.v1|import ommx\.v1|ommx\.v1\.' python docs
  ```

When reviewing CI failures, fetch the exact failing Sphinx or doctest log before
changing docs. If a doctest imports packages by discovery, check whether a
sentinel package is intentionally import-failing for migration guidance.

## Findings

Lead review findings with user-visible publication risk:

- Missing or stale API reference entry.
- Generated artifacts not refreshed from the handwritten source.
- Current examples import an obsolete public API.
- Migration docs confuse Python import namespaces with protobuf wire namespaces.
- Sphinx build warnings that would fail CI.
- Docstrings that render incorrectly or break AutoAPI/docutils parsing.

Keep generated-diff summaries secondary. A large generated diff is acceptable
when the owner configuration and regeneration path are correct.
Successful generation or a warning-free build does not establish that a reader
can understand the page; report only the technical properties checked here.
