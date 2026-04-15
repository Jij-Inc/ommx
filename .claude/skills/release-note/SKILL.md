---
name: release-note
description: Append release note entries for the current PR to both English and Japanese release notes
argument-hint: "[version e.g. 3.0]"
disable-model-invocation: true
allowed-tools: "Bash(git *) Bash(gh *) Read Edit"
---

# Release Note Writer

Write release note entries for the current branch's PR in both English and Japanese.

## Scope

This is the **Python SDK** release note. Only include changes visible to Python SDK users:
- New Python API, classes, methods, properties
- Behavior changes in Python SDK
- Bug fixes affecting Python users
- Adapter changes

Do NOT include:
- Rust SDK internal changes (unless they surface as Python API changes)
- CI/CD, documentation-only, or tooling changes
- CLAUDE.md or development workflow changes

## Steps

1. Determine the target version from `$ARGUMENTS` (e.g. "3.0"). If not provided, infer from the latest file in `docs/en/release_note/`.

2. Identify the current PR:
   ```
   gh pr view --json number,title,body,url
   ```

3. Understand what changed by reviewing the full diff against main:
   ```
   git diff main...HEAD
   ```

4. Read the existing release note files:
   - `docs/en/release_note/ommx-{version}.md`
   - `docs/ja/release_note/ommx-{version}.md`

5. Append entries to both files following the existing format:
   - Use `###` headings with PR link: `### Feature name ([#NNN](https://github.com/Jij-Inc/ommx/pull/NNN))`
   - Place under appropriate `##` section (New Features, Bug Fixes, Breaking Changes, etc.)
   - Include code examples if the change adds new API
   - English first, then write the Japanese version as a natural translation (not machine-translated tone)

6. Show the user the diff of what was added for review.

## Format reference

English:
```markdown
### Feature name ([#123](https://github.com/Jij-Inc/ommx/pull/123))

Description of the change from the user's perspective. Include code examples for new API.
```

Japanese:
```markdown
### 機能名 ([#123](https://github.com/Jij-Inc/ommx/pull/123))

ユーザー視点での変更の説明。新しいAPIにはコード例を含める。
```
