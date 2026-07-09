---
name: ommx-issue-triage
description: Use when triaging, labeling, closing, rewriting, or organizing GitHub issues in Jij-Inc/ommx, especially when deciding consistent issue labels, separating issue backlog labels from PR release-note labels, auditing open issues, or applying GitHub issue metadata changes.
---

# OMMX Issue Triage

Use this skill to manage OMMX GitHub issues consistently. Keep the live GitHub
state as the source of truth; do not rely on stale memory, old issue summaries,
or local assumptions when labels, open/closed state, assignees, or milestones
matter.

## Scope

Apply this skill to issues, not PR release preparation. When the task is to
label a PR for release notes, hand off to the PR review or publish workflow and
judge labels from the actual diff.

Use labels rather than GitHub Issue type for normal issue management in this
repo. Current GitHub CLI versions may expose issue type, but OMMX's triage and
release workflows are label-based, and CLI availability varies across agent
environments.

## Label Model

Treat labels in two groups. Keep label names short and stable; improve
consistency through descriptions and usage rules before creating new labels.

Release-note labels are shared with PRs and are interpreted by
`.github/release.yml`. Do not rename, delete, or change the meaning of these
labels during issue triage:

- `breaking change`: accepted work intentionally breaks compatibility.
- `bug`: observed incorrect behavior or failing workflow.
- `proto`: accepted work changes protobuf schema or generated protobuf contract.
- `python`: PR changes the Python SDK public API, generated stubs, or adapter
  public API.
- `rust`: PR changes the Rust SDK public API.
- `documentation`: accepted work is primarily docs, examples, tutorials,
  migration guides, or API reference.
- `dependencies`: dependency update work; GitHub release notes exclude it.

Non-release labels are decision aids. Design them from the viewpoint of who
uses the label and what decision it supports; do not treat them as a flat
taxonomy.

Reporter-facing labels classify what kind of problem a library user is
reporting:

- `feature request`: user tried or planned to use OMMX and needs a missing
  capability, API, format support, integration, or workflow.
- `performance`: existing supported behavior works, but is slower, larger, or
  less scalable than expected. Require benchmarks before claiming improvement.
- `bug` is also reporter-facing, but it is a release-note label because bug-fix
  PRs must be categorized in releases.

Do not apply `rust` or `python` to issues. Those labels answer what API surface
actually changed in a PR; an issue can discuss likely implementation areas
without carrying a speculative API-change label.

Developer-facing labels classify the implementation work:

- `maintenance`: internal quality, repository operation, tooling, linting,
  cleanup, migration hygiene, or development workflow work that is neither a
  user feature nor an observed bug.
- `rfc`: direction-setting issue used to reach agreement on API, schema,
  architecture, semantics, responsibility boundary, or scope. Use this when the
  outcome may be an accepted direction, rejection, or follow-up implementation
  issues rather than a single concrete implementation PR.

Manager-facing labels classify backlog handling:

- `workstream`: parent issue that tracks progress across multiple PRs, child
  issues, or implementation slices. Do not use for single-PR work.
- `needs info`: cannot be assigned or scoped until reproduction details,
  expected behavior, affected version, or acceptance criteria are clarified.
- `blocked`: work is accepted but waiting on an external dependency, upstream
  decision, credential/permission change, or prerequisite PR.
- `help wanted`: scoped enough for an external contributor; maintainers welcome
  help.
- `good first issue`: small, low-risk, and well-specified enough for a new
  contributor.

Triage-outcome labels close or redirect work:

- `duplicate`: same underlying work is tracked elsewhere.
- `wontfix`: the work will not be pursued.

Automation labels such as `github_actions` and `python:uv` are Dependabot-owned
PR labels in this repository. Ignore them during issue triage and do not apply
them manually to issues. If Dependabot noise becomes a problem, fix the
Dependabot configuration separately instead of repurposing these labels.

Do not add domain labels such as `artifact`, `experiment`, `adapter`, or
`dataset` just because an issue belongs to that area; those topics are
searchable from titles and bodies. Propose a new domain label only after several
active issues need the same planning queue and existing labels cannot express it
without overloading release-note semantics.

Prefer GitHub Projects or milestones over labels for priority, owner, status,
and due date. Labels should remain stable filters, not a second project board.

## Issue Relationships

Use GitHub issue Relationships for strict issue-to-issue blocking
dependencies. Treat them as the machine-readable dependency graph, not as a
general "related issue" list.

Add a Relationship when:

- issue B cannot be implemented safely until issue A is decided or completed;
- doing B before A would likely cause rework or produce an invalid API shape;
- maintainers need the dependency graph to decide whether B is ready to start.

Do not add a Relationship for loose topical links, follow-up reading,
alternative proposals, same-area work, or "nice to do together" coordination.
Use issue links in the body or comments for those cases.

Keep the rationale outside the Relationship itself:

- use the issue body for workstream status, child issue lists, accepted
  decisions, and remaining work;
- use comments for the audit trail explaining why a dependency was added,
  removed, or reclassified;
- use `workstream` for the parent issue that tracks progress across multiple
  PRs or child issues, even when some children also have Relationships.

The `blocked` label is broader than Relationships: use it when accepted work is
waiting on an external dependency, upstream decision, permission, credential, or
prerequisite PR. Use Relationships only for GitHub issue-to-issue dependencies.

Prefer the supported `gh issue` fields and flags when the local GitHub CLI
exposes them:

```bash
gh issue view 123 --repo Jij-Inc/ommx --json number,title,blockedBy,blocking
gh issue edit 456 --repo Jij-Inc/ommx --add-blocked-by 123
gh issue edit 123 --repo Jij-Inc/ommx --add-blocking 456
```

Before mutating Relationships, restate the exact dependency edge, for example
"`#123` blocks `#456`". Use `--remove-blocked-by` or `--remove-blocking` to
remove the same edge.

Some agent environments have older `gh` versions whose `issue view --json` and
`issue edit` commands do not expose Relationship fields or flags. In that case,
fall back to GraphQL for reads:

```bash
gh api graphql -f query='
query($owner:String!, $repo:String!, $number:Int!) {
  repository(owner:$owner, name:$repo) {
    issue(number:$number) {
      number
      title
      issueDependenciesSummary { totalBlockedBy totalBlocking }
      blockedBy(first:50) {
        totalCount
        nodes { number title url repository { nameWithOwner } }
      }
      blocking(first:50) {
        totalCount
        nodes { number title url repository { nameWithOwner } }
      }
    }
  }
}' -f owner=Jij-Inc -f repo=ommx -F number=123
```

Use raw GraphQL mutations only when the supported `gh issue edit` flags are not
available in the local CLI.

## Recommended GitHub Descriptions

When brushing up labels on GitHub, prefer description-only updates unless the
user explicitly wants renames or color changes:

| Label | Recommended description |
| --- | --- |
| `breaking change` | Accepted work intentionally breaks compatibility |
| `bug` | Observed incorrect behavior or failing workflow |
| `dependencies` | Dependency update work; excluded from release notes |
| `blocked` | Accepted work is waiting on an external dependency or prerequisite |
| `documentation` | Docs, examples, tutorials, migration guides, or API reference |
| `duplicate` | Same underlying work is tracked elsewhere |
| `feature request` | User needs a missing capability, API, format support, integration, or workflow |
| `good first issue` | Small, well-scoped, low-risk issue for new contributors |
| `help wanted` | Maintainers welcome external help on this scoped issue |
| `maintenance` | Internal quality, repository operation, tooling, linting, or cleanup work |
| `needs info` | Needs reproduction details, expected behavior, version, or acceptance criteria |
| `performance` | Performance, scaling, storage size, streaming, or repeated-cost concern |
| `proto` | Protobuf schema or generated protobuf contract changes |
| `python` | PR changes the Python SDK public API, generated stubs, or adapter public API |
| `rfc` | Direction-setting discussion for API, schema, architecture, semantics, or scope |
| `rust` | PR changes the Rust SDK public API |
| `workstream` | Parent issue tracking multiple PRs, child issues, or implementation slices |
| `wontfix` | This work will not be pursued |

## Triage Workflow

1. Resolve live state.
   - Read the current labels:
     `gh label list --repo Jij-Inc/ommx --limit 100 --json name,description,color`.
   - Read current open issues:
     `gh issue list --repo Jij-Inc/ommx --state open --limit 100 --json number,title,labels,assignees,milestone,updatedAt,createdAt,url`.
   - For unlabeled, stale, or ambiguous issues, fetch the issue body before
     classifying. Titles alone are not enough.
   - For dependency audits, read `blockedBy` and `blocking` through
     `gh issue view --json number,title,blockedBy,blocking` when available.
     If the local CLI reports unknown fields, fall back to GraphQL.

2. Classify by audience and decision.
   - Reporter-facing: is this a missing capability (`feature request`), an
     observed failure (`bug`), or a speed/scale problem (`performance`)?
   - Developer-facing: is this direction-setting (`rfc`), or internal
     quality/tooling/process work (`maintenance`)?
   - Manager-facing: is this a multi-PR parent (`workstream`), waiting for
     information (`needs info`), blocked (`blocked`), suitable for outside help
     (`help wanted`), or suitable for a newcomer (`good first issue`)?
   - Ask what code/docs would change if this issue were implemented.
   - Do not add `rust` or `python` to issues based on expected implementation
     surface. Apply those labels only to PRs after checking the actual diff and
     confirming an SDK API change.
   - If the issue carries API, architecture, invariant, or scope tradeoffs but
     also has concrete acceptance criteria for a single implementation task, do
     not add a separate label only for those tradeoffs; keep the reasoning in
     the issue or PR body.
   - If the issue describes an observed failure, use `bug` even when the eventual
     fix will touch `rust`, `python`, `documentation`, or repository workflows.

3. Treat `feature request`, `rfc`, and `workstream` as different questions.
   - `feature request` answers: what new user-visible capability or workflow is
     being requested?
   - `rfc` answers: which API, schema, architecture, semantic, responsibility, or
     scope direction should be accepted before implementation issues are split
     out or PR work proceeds?
   - `workstream` answers: where do we track progress, decisions, and remaining
     work across multiple PRs or child issues?
   - Use `feature request` for concrete implementation tasks even when the body
     includes design questions.
   - Use `rfc` for direction-setting documents whose natural result is a
     decision or follow-up issues, not one direct PR.
   - Use `workstream` for parent issues with status tables, checklists, child
     issues, or multi-PR progress tracking.

4. Preserve PR release-note semantics.
   - Do not use `rust`, `python`, `proto`, `documentation`, or `bug` as vague
     topic tags.
   - Do not apply `rust` or `python` during issue triage. Use them only in PR
     review/release-note workflows after the actual API change is visible.
   - Do not relabel open PRs from this skill unless the user explicitly asks; PR
     labels should be judged from the actual diff.

5. Propose before mutating.
   - Unless the user explicitly asks to apply changes immediately, first show a
     table with issue number, current labels, proposed labels, and reason.
   - Separate additions from removals.
   - Mark ambiguous cases and explain what evidence would decide them.

6. Apply changes narrowly.
   - Before a write, restate the exact issue number and label changes.
   - Prefer additive label changes when the existing label is still correct.
   - Remove a label only when it conflicts with the model above.
   - Before mutating Relationships, restate the exact dependency edge, for
     example "`#1007` blocks `#1030`".
   - Do not encode loose related links as Relationships; use body links or
     comments instead.
   - Do not close, reopen, assign, or edit issue bodies as part of label cleanup
     unless the user asks for that action.

## Common OMMX Judgments

- Do not use `rust` or `python` to say "this issue probably touches that part of
  the codebase." Those labels mean the reviewed PR changed the Rust or Python
  SDK API.
- Migration guides, Sphinx pages, API-reference wiring, tutorials, and example
  prose may use `documentation` when the issue itself is documentation work.
- Wire-format schema, protobuf root messages, generated message compatibility,
  and Buf publication work may use `proto` when the issue is explicitly about
  the protobuf contract.
- GitHub token permissions, PR workflows, CI jobs, release automation, and
  registry push jobs driven by Actions imply `bug` when they are failing
  unexpectedly. Do not use `github_actions` for issue triage; it is a
  Dependabot PR label in this repository.

## Output

For audit-only requests, summarize counts and the highest-leverage cleanup
groups. For action requests, return the applied changes and any skipped
ambiguous issues. Keep the output issue-focused and avoid release-note prose
unless PR labeling is explicitly in scope.
