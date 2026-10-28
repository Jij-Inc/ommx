---
name: book-reading-review
description: Review OMMX Book tutorials, guides, and navigation for reader prerequisites, introduction order, cognitive load, independent reading paths, and explanation placement between the Book and API Reference. Also audit major SDK feature coverage when requested. Use for those explanations and learning paths, rather than for generated API wiring or Sphinx build failures alone.
---

# Book Reading Review

Review what a reader can understand and do at each point, not just whether the
prose is technically correct. OMMX defines problem and candidate semantics
independently of a solver. Keep that common model separate from an Adapter's
execution contract and the guarantees of an individual transformation.

This workflow can stand alone for a reader-comprehension question. A change
limited to API generation, directives, or Sphinx build/render/link correctness
belongs to `sphinx-docs-review` and does not require this skill or its ledger.
Apply both only when the request includes both concerns; do not load the other
skill merely because a Book file changed.

## Establish the reader and source

- Resolve the requested PR/revision and read its actual files. A local checkout
  may contain a later stacked PR; do not review that as the requested change.
- Identify the reader's goal, starting knowledge, entry link, and useful stopping
  point. Knowing Python and ordinary optimization does not imply knowing OMMX
  types, retained variables, removed constraints, or Preparation.
- Follow the entry that the navigation promises, including direct Guide/How-to
  entry and alternative routes. Do not assume the reader completed every prior
  Tutorial. A declared prerequisite is different from a recommended link.
- Separate conceptual knowledge from execution dependencies. Hidden cells still
  require their imports, input data, and prior runtime state.

Notebook execution correctness is covered by the repository's separate tests.
Do not add Notebook reruns to a Book review. Inspect the example and its displayed
outputs for explanatory sufficiency; runtime test failures belong to the test
workflow. Keep documented SDK versions and prerequisites accurate without
treating a Book build as execution evidence.

## Audit feature coverage when requested

For a whole-Book coverage audit, start from the public SDK at the documented
version and the reader tasks it supports, including tasks absent from the Book
or ledger. Group APIs by useful operations rather than requiring a page for
every method. Check the public bindings/exports against their owning
implementation; distinguish released capabilities from development-only APIs.

For each major task, locate its conceptual explanation, usable example, and
entry path. Record source pointers and whether coverage is usable, only
mentioned, planned, absent, or deliberately excluded. A release note, API entry,
or planned ledger owner alone does not supply a Book learning path. Distinguish
missing implementation of an existing plan from a proposed expansion of scope.

Reconcile gaps with the accepted workstream scope and explanation ownership
below. Keep feature inventories in the review record or Book-owned ledger,
not in this skill. Ordinary page/PR reviews remain limited to affected tasks;
they do not require a whole-SDK inventory.

## Inspect introductions and uses

For the affected passage, classify each relevant concept as an entry
prerequisite, local introduction, understanding-dependent use, or mere mention.
Read enough surrounding text and the linked explanation to justify the choice.

- A definition, a small example with its interpretation, or a short local gloss
  can introduce the capability needed here. The detailed owner page need not be
  mandatory reading. Its link alone does not teach the concept.
- A type name, repr field, output column, navigation label, or optional section
  does not establish mastery. Conversely, an unfamiliar API name is not itself a
  finding when the example and adjacent explanation make its role clear.
- Check the capability's scope. Reading a Solution's values is not the same as
  understanding transformed feasibility or optimality. A Binary example does
  not teach SemiInteger/SemiContinuous domains. Split an overly broad concept
  when a real use needs a narrower capability, not for every API spelling.
- Inspect order inside a section, not just page order. A reference from an early
  passage to a later page that requires the unfinished passage's knowledge can
  leave a learning gap. Two pages linking to each other is not, by itself, a
  prerequisite cycle.
- Keep the learning budget proportional to the task. A correct overview can
  still overload an entry with unexplained names. Move nonessential mechanics
  after the first success instead of adding a glossary before it.

## Inspect explanation ownership

For each affected page, identify what the passage enables its reader to
understand or do. Inspect retained passages as well as newly added lines.

Tutorials teach one practical workflow; How-tos complete an independent task;
Guides own concepts and interpretation. Keep the conceptual distinctions,
minimal examples, and safety summaries needed for that task in the Book.
API Reference owns exact argument/default rules, exhaustive cases, execution
order, formulas, preconditions, errors, mutation guarantees, and numerical
contracts. Value reconstruction, feasibility, objective values, and optimality
are distinct guarantees.

Overlap alone is not a finding. A short safe-use warning or a counterexample
teaching an operation's meaning may be necessary even when the API states the
same fact. Flag repetition when it adds contract detail without serving the
page's learning goal; neither mathematical notation nor an API name alone
determines where an explanation belongs.

For a relocation, identify what understanding the Book must retain. If the
contract is missing from the API Reference, report or fill that documentation
gap within the authorized scope before removing its only explanation.

When moving or removing an explanation, read its actual replacement and follow
the affected incoming entry through to the definition or usable procedure.
A page, heading, anchor, or resolving link is not evidence that the explanation
exists. Follow onward references until the reader reaches the needed content;
do not accept a chain that only sends them back to the removed explanation.
The replacement must be available in the reviewed PR/revision, not only in a
later stacked PR. Report an unmet route rather than recommending Ready while
its replacement is still planned.

## Surface API friction separately

Do not absorb an awkward public API into increasingly elaborate explanations.
When an ordinary reader task needs raw metadata keys, repeated low-level
selection, or surprising call semantics, distinguish:

- a missing introduction or poor explanation, which belongs in the Book;
- a necessary domain distinction, which must be taught rather than hidden;
- API friction, where the supported operation itself makes the task needlessly
  difficult or its effects hard to discover.

Verify an API candidate against the current implementation and a minimal user
example, not only the length of its explanation. Respect accepted design
tradeoffs and explicit exclusions; do not reopen them without new evidence.
Search existing issues and PRs first, linking relevant work rather than
creating duplicates.

For a confirmed new API problem, file a separate follow-up issue when the task
authorizes issue creation, using `ommx-issue-triage`. In a read-only review,
report the candidate and request authority before posting. Include the reader's
goal, expected versus actual behavior, minimal code, source revision, affected
Book passage, and current workaround. Describe the missing capability and
domain constraints without treating a proposed API signature as decided.

Link the follow-up from the review record. Keep the Book's current workaround
accurate and runnable; do not silently redesign the SDK in a Book PR. An
ergonomic follow-up need not block the Book when the documented workflow is
correct, but filing an issue does not excuse an incorrect example or promise.

## Reconcile changed explanations with the ledger

The Book-owned data is `docs/_meta/reading_paths/manifest.json`, not part of the
skill's instructions. It starts as a proposed design based on an explicitly
recorded revision, not a certification of the current Book.

For each changed page affecting prerequisites, introductions, or navigation,
compare the actual prose and Notebook cells with the affected concept, page,
and route contracts. Locate the introduction and first understanding-dependent
use; check their order, scope, and entry knowledge against the declarations.
This comparison is required, not optional because the analyzer passes. If the
page or concept is untracked, record that coverage gap. With editing authority,
update the affected contracts; in a review-only task, report the mismatch
without rewriting them. Query only the affected subset, not the whole Book.
The query helper returns declarations alongside whole-manifest diagnostics,
even when contracts are inconsistent. Use those diagnostics to interpret the
target; successful lookup is not a consistency or readiness verdict.

The helper commands below are optional tools for inspecting the declarations;
they do not replace the required comparison. If used, run them from the
repository root through its Python environment:

```sh
uv run --no-sync python .agents/skills/book-reading-review/scripts/query.py docs/_meta/reading_paths/manifest.json concept class.membership
uv run --no-sync python .agents/skills/book-reading-review/scripts/query.py docs/_meta/reading_paths/manifest.json page P11
uv run --no-sync python .agents/skills/book-reading-review/scripts/analyze.py docs/_meta/reading_paths/manifest.json
```

Use the environment/version appropriate to the checkout and honor its execution
permissions. These helpers use only the standard library and no LLM or network.
They check declared sets and dependencies, not natural-language comprehension.

Before editing contracts or using a source-evidence bundle, read
[the schema and evidence rules](references/schema.md). Keep Book-specific
concepts, owners, and paths in the ledger rather than copying them into this
skill. A review-only request does not authorize rewriting the ledger or posting
findings externally.

The analyzer teaches a section's ordered `introduces` before checking its
`uses`. Represent a use before an explanation as an earlier section; putting
both into one section can hide the defect. `mentions` never teaches, optional
children are not inherited, and route joins retain only common knowledge.

## Report evidence, not a mechanical readiness verdict

For each finding, name the reader/route, the concrete use and source position,
the capability not yet available, and the smallest change that restores the
workflow. Distinguish an omitted explanation from merely unfamiliar spelling.
Prefer a local gloss, a reorder, or deferring an unnecessary detail over making
the reader complete another whole Guide.

For an explanation-placement finding, identify the reader's question, the
detail's canonical owner, and the smallest Book explanation to retain. When
reviewing a PR, distinguish newly introduced problems from inherited cleanup
candidates in the affected pages.

Keep these outcomes separate:

- observed text and outputs at the reviewed revision;
- consistency of the proposed learning contracts;
- manual review of explanatory sufficiency and entry-page cognitive load;
- code execution, version compatibility, and mathematical/API correctness.

Before concluding, leave a concise [current-revision review record](references/schema.md#current-revision-review-record)
in the existing review response or work notes. Identify the reviewed revision,
paragraphs or Notebook cells, reader routes, and actual introduction/use and
destination evidence. Distinguish checked coverage from unresolved or unreviewed
routes; do not create a whole-Book PASS report for a scoped review.

Recommend or mark Ready, within the task's authorization, only after the
affected prose/contracts have been reconciled, the review record is present,
and moved explanations have usable destinations at that revision. A contract
PASS or a warning-free Sphinx build cannot satisfy those conditions. Keep the
reading review incomplete when an affected route still has a gap.
Build/API-reference checks remain the responsibility of the existing Sphinx
workflow; do not broaden this review into SDK changes or a Book-wide redesign.
