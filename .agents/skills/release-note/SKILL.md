---
name: release-note
description: Append release note entries for the current PR to the relevant OMMX Python SDK and/or Rust SDK release notes
---

# Release Note Writer

Write release note entries for the current branch's PR in the relevant SDK release notes.

## Scope

OMMX has separate release note surfaces:

- **Python SDK**: `docs/en/release_note/ommx-{version}.md` and `docs/ja/release_note/ommx-{version}.md`
- **Rust SDK**: `rust/ommx/doc/release_note.md`

Add entries only to the surfaces affected by the PR.

For the **Python SDK** release note, include changes visible to Python users:
- New Python API, classes, methods, properties
- Behavior changes in Python SDK
- Bug fixes affecting Python users
- Adapter changes

For the **Rust SDK** release note, include changes visible to Rust crate users:
- New or changed public Rust API
- Behavior changes in the Rust SDK
- Bug fixes affecting Rust SDK users
- Serialization, schema, or error-surface changes exposed through Rust APIs

Do NOT include:
- Rust SDK internal changes that do not affect the public Rust API or user-visible behavior
- Python SDK internal changes that do not affect Python users
- CI/CD, documentation-only, or tooling changes
- AGENTS.md or development workflow changes

## Steps

1. Identify the current PR:
   ```
   gh pr view --json number,title,body,url
   ```

2. Understand what changed by reviewing the full diff against main:
   ```
   git diff main...HEAD
   ```

3. Decide which release note surfaces are affected:
   - Python SDK changes go to both language files:
     - `docs/en/release_note/ommx-{version}.md`
     - `docs/ja/release_note/ommx-{version}.md`
   - Rust SDK changes go to:
     - `rust/ommx/doc/release_note.md`
   - If a PR affects both SDKs, update all affected files.

4. Determine the target Python SDK version only if Python release notes are
   affected. Use `$ARGUMENTS` (e.g. "3.0") when provided; otherwise infer from
   the latest file in `docs/en/release_note/`.

5. Read the existing release note files for the affected surfaces. For Rust-only
   changes, do not edit the Python release note files.

6. Check whether the same user-facing behavior is already explained in Tutorial
   or User Guide pages under `docs/en/` and `docs/ja/`. When detailed docs
   already exist, keep the release note concise and link readers to the
   relevant Tutorial/User Guide section instead of duplicating the full explanation.
   For Rust SDK changes, also check `rust/ommx/doc/` and prefer concise links to
   existing Rust docs such as the migration guide when appropriate.

7. Append Python SDK entries to both language files following the existing format:
   - Use `###` headings with PR link: `### Feature name ([#NNN](https://github.com/Jij-Inc/ommx/pull/NNN))`
   - Place under appropriate `##` section (New Features, Bug Fixes, Breaking Changes, etc.)
   - Include code examples if the change adds new API
   - English first, then write the Japanese version as a natural translation (not machine-translated tone)

8. Append Rust SDK entries to `rust/ommx/doc/release_note.md` following the existing format:
   - Use `##` headings with PR link: `## Feature name ([#NNN](https://github.com/Jij-Inc/ommx/pull/NNN))`
   - Write in English for docs.rs.
   - Use Rustdoc links such as ``[`Instance`](crate::Instance)`` for public Rust items.
   - Keep the entry focused on the Rust user's migration or behavior impact.

9. Show the user the diff of what was added for review.

## Format reference

Python English:
```markdown
### Feature name ([#123](https://github.com/Jij-Inc/ommx/pull/123))

Description of the change from the user's perspective. Include code examples for new API.
```

Python Japanese:
```markdown
### 機能名 ([#123](https://github.com/Jij-Inc/ommx/pull/123))

ユーザー視点での変更の説明。新しいAPIにはコード例を含める。
```

Rust:
```markdown
## Feature name ([#123](https://github.com/Jij-Inc/ommx/pull/123))

Describe the Rust SDK API or behavior impact. Use Rustdoc links such as
[`Instance`](crate::Instance) when naming public items.
```
