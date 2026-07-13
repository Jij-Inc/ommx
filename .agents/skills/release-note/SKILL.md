---
name: release-note
description: Append release note entries for the current PR to the relevant OMMX Python SDK and/or Rust SDK release notes
---

# Release Note Writer

Write release note entries for the current branch's PR in the relevant SDK release notes.

## Scope

OMMX has separate release note surfaces:

- **Python SDK**: `docs/en/release_note/ommx-{version}.md` and `docs/ja/release_note/ommx-{version}.md`
- **Rust SDK**: release-line pages under
  `rust/ommx/doc/release_note/{major}.{minor}.md` (for example,
  `rust/ommx/doc/release_note/3.0.md`).

`rust/ommx/doc/release_note.md` is only the release-line index. Update it when
adding or renaming a release line, but never append individual release entries
to the index.

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
   - Rust SDK changes go to the affected release-line page, for example:
     - `rust/ommx/doc/release_note/3.0.md`
   - If a PR affects both SDKs, update all affected files.

4. Apply GitHub Release labels to the PR before writing entries.
   - Read and follow `../ommx-pr-labeling/SKILL.md`.
   - Audit the live PR diff, `.github/release.yml`, and current PR labels, then
     apply every semantically correct Release label while preserving unrelated
     labels.
   - Use the resulting labels to choose the release-note surfaces and entry
     categories. Resolve any mismatch between the labels and the proposed
     release-note placement before editing the notes.

5. Determine the target SDK release line for every affected surface.
   - For Python, use `$ARGUMENTS` (e.g. "3.0") when provided; otherwise infer
     from the latest file in `docs/en/release_note/`.
   - For Rust, select the matching file under
     `rust/ommx/doc/release_note/`. Use `rust/ommx/doc/release_note.md` only to
     confirm how release-line pages are routed from the index.

6. Read the existing release note files for the affected surfaces. For Rust-only
   changes, do not edit the Python release note files. Preserve the structure
   declared at the top of the selected Rust release-line page.

7. Check whether the same user-facing behavior is already explained in Tutorial
   or User Guide pages under `docs/en/` and `docs/ja/`. When detailed docs
   already exist, keep the release note concise and link readers to the
   relevant Tutorial/User Guide section instead of duplicating the full explanation.
   For Rust SDK changes, also check `rust/ommx/doc/` and prefer concise links to
   existing Rust docs such as the migration guide when appropriate.

8. Append Python SDK entries to both language files following the existing format:
   - Use `###` headings with PR link: `### Feature name ([#NNN](https://github.com/Jij-Inc/ommx/pull/NNN))`
   - Place under appropriate `##` section (New Features, Bug Fixes, Breaking Changes, etc.)
   - Within each version section, order entries by user impact: Bug Fixes,
     Breaking Changes, then New Features. Place other categories after these
     unless the existing release section requires a more specific grouping.
   - Within each category, use merge order by default, with the most recently
     merged change first.
   - Include code examples if the change adds new API
   - English first, then write the Japanese version as a natural translation (not machine-translated tone)

9. Update the selected Rust SDK release-line page following its existing
   lifecycle structure:
   - Before the stable `{major}.{minor}.0` release, consolidate alpha, beta,
     and release-candidate changes by topic in the main body above the final
     `{major}.{minor}.x updates` section. Do not add one section per PR; record
     provenance in the topic's `Related PR` / `Related PRs` line.
   - After the stable `{major}.{minor}.0` release, append PR-based entries under
     the final `## {major}.{minor}.x updates` section. Use a nested `###`
     heading with the PR link:
     `### Feature name ([#NNN](https://github.com/Jij-Inc/ommx/pull/NNN))`.
   - Preserve any topic markers or category conventions defined by the
     release-line page.
   - Write in English for docs.rs.
   - Use Rustdoc links such as ``[`Instance`](crate::Instance)`` for public Rust items.
   - Keep the entry focused on the Rust user's migration or behavior impact.

10. Show the user the diff of what was added for review.

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

Rust after the stable release, nested under `## 3.0.x updates`:
```markdown
### Feature name ([#123](https://github.com/Jij-Inc/ommx/pull/123))

Describe the Rust SDK API or behavior impact. Use Rustdoc links such as
[`Instance`](crate::Instance) when naming public items.
```
