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
   - For docstring changes, review the Rust/Python source docstrings that feed
     generated stubs and `docs/api/api_reference.json`.

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

## Validation Commands

Use the narrowest commands that match the diff. Python commands must run through
the project environment.

- After PyO3 public API or docstring changes:
  ```
  task python:stubgen
  ```
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
