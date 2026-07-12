---
name: ommx-pr-labeling
description: Use when deciding, auditing, or applying GitHub Release labels on OMMX pull requests, especially `rust`, `python`, `proto`, `lean`, `documentation`, `bug`, or `breaking change` labels for release-note generation from the actual PR diff.
---

# OMMX PR Labeling

## Overview

Use this skill to classify OMMX PR labels consumed by `.github/release.yml`.
Judge labels from the live PR diff and current release configuration, not from
issue labels, implementation guesses, or stale memory.

## Ground Rules

- Treat `.github/release.yml` as the source of truth for release label names and
  categories.
- Treat PR labels as release-note impact labels, not topic tags or code-owner
  tags.
- Inspect the actual diff against the PR base branch before deciding `rust`,
  `python`, `proto`, `lean`, or `documentation`.
- Apply `bug` and `breaking change` from the latest stable OMMX release user's
  viewpoint, because these labels must make sense in user-facing release notes.
- Apply all labels that are semantically true. Do not omit a true label just to
  force one release category unless the maintainer explicitly asks for that.
- Do not use this skill for GitHub issue triage. Use `ommx-issue-triage` for
  issue labels and Relationships.
- When a user says `document` as a PR tag, map it to the GitHub label
  `documentation`.

## Label Rules

Use the canonical labels below. If `.github/release.yml` has changed, follow the
live file and explain the difference.

| Label | Apply when | Do not apply when |
| --- | --- | --- |
| `rust` | The Rust SDK public API or Rust-user-visible behavior changes. Examples: public types, methods, traits, parse/evaluation/serialization semantics, error behavior, or rustdoc-visible SDK commitments. | The PR only refactors private Rust code, changes tests, touches build tooling, or changes Rust internals with no Rust SDK user impact. |
| `python` | The Python SDK or adapter public API or Python-user-visible behavior changes. Examples: top-level `ommx` exports, PyO3 bindings, generated `.pyi` stubs that reflect a real API change, adapter APIs, or Python-visible semantics/errors. | The PR only changes Rust internals, test fixtures, formatting, or generated artifacts without a real Python API or behavior change. |
| `proto` | `proto/` schema or the generated protobuf contract changes. Examples: message fields, enums, field numbers, wire-format compatibility, or Buf-published schema behavior. | The PR only documents protobuf concepts without changing the schema or generated protobuf contract. |
| `lean` | At least one non-generated Lean source line under `lean/**/*.lean` changes. This includes the formal AST, normalization or denotation, executable checkers, theorem statements or proofs, and Lean test fixtures. | The PR changes only the Lake manifest, toolchain, dependency or build configuration, CI, Taskfiles, or Lean documentation without changing a `.lean` source file. |
| `documentation` | The PR is documentation-only: Sphinx pages, migration guides, tutorials, examples, rustdoc prose, release notes, or API reference wiring with no code/proto/Lean behavior change. | Docs accompany a Rust/Python/proto/Lean change. In that case, label the changed surface instead of adding `documentation` merely because docs were updated. |
| `bug` | The PR fixes behavior that was wrong in the latest stable OMMX release and should be reported to users as a bug fix. This may combine with `rust`, `python`, or `proto` when the stable-release bug affected those surfaces. | The PR only fixes behavior that existed on `main`, in a prerelease such as alpha, or after the latest stable release and was never shipped to stable-release users; the PR is cleanup, a feature, docs-only work, or preventive hardening without a stable-release-user-visible failure. |
| `breaking change` | The PR intentionally breaks compatibility with the latest stable OMMX release or requires stable-release users to migrate. Combine it with the affected surface labels. | The change is additive or internal, only breaks unreleased work on `main`, only changes a prerelease or alpha artifact/API before the next stable release, or would not require migration for users coming from the latest stable release. |
| `dependencies` | A dependency update PR should be excluded from release notes according to `.github/release.yml`. Leave Dependabot-owned labels alone unless the user asks. | Ordinary feature, bug-fix, docs, schema, or SDK work. |

## Workflow

1. Resolve live PR state.
   - Read the release label configuration:
     `sed -n '1,200p' .github/release.yml`
   - Read the current PR and labels:
     `gh pr view --json number,title,labels,baseRefName,headRefName,url`
   - Review the actual diff against the base branch. Prefer the PR base from
     `gh pr view`; `main...HEAD` is acceptable when the base is `main`.

2. Classify by user-visible impact.
   - Ask which public contract changed: Rust SDK, Python SDK/adapters, protobuf
     schema, Lean formalization, documentation, bug fix, or breaking migration.
   - Do not classify from file paths alone. A Rust file can change Python
     behavior through PyO3, and a Python-facing API change can require generated
     Rust or stub updates.
   - For generated files, trace back to the source change. Generated stubs or
     docs can confirm a public API change, but pure regeneration noise is not
     enough.

3. Apply the `bug` and `breaking change` labels only from a
   stable-release-user viewpoint.
   - Before adding `bug`, identify whether the bad behavior existed in the
     latest stable OMMX release for the affected surface. Check release notes,
     stable tags, or the PR/issue history when this is not obvious from the
     diff.
   - Before adding `breaking change`, identify whether the PR breaks a contract
     that existed in the latest stable OMMX release or requires users migrating
     from that stable release to change code, data, artifacts, or workflows.
   - Use `bug` only when a user who stayed on the latest stable release could
     have hit the bad behavior and should see the PR summarized as a bug fix in
     GitHub Release notes.
   - Use `breaking change` only when a user coming from the latest stable
     release needs a migration warning in GitHub Release notes.
   - Do not use `bug` or `breaking change` for corrections to unreleased work
     already on `main`, prerelease-only regressions or format changes such as
     alpha-to-alpha fixes, follow-up fixes before the next stable release,
     test-only failures, or internal consistency fixes that no stable-release
     user could observe.

4. Separate docs-only from docs-accompanying-code.
   - If the PR only changes docs, examples, migration text, release notes, or
     API reference wiring, use `documentation`.
   - If docs were updated to explain a Rust/Python/proto/Lean change, do not add
     `documentation` unless the maintainer explicitly wants mixed docs labels.

5. Propose or apply labels narrowly.
   - For audit requests, show current labels, proposed additions/removals, and a
     one-line evidence note for each release label.
   - For action requests, mutate only the release labels needed for this PR.
     Preserve unrelated labels.
   - Prefer `gh pr edit <number> --add-label <label>` and
     `gh pr edit <number> --remove-label <label>` for writes.

## Output

Return the final label set and the evidence used to decide it. Call out
ambiguous cases explicitly, especially when an internal implementation change
may or may not be visible to Rust or Python users.
