---
name: book-reading-review
description: Plan, write, or review OMMX README and Book explanations of purpose, adoption value, capabilities, reader choices, and independent reading paths. Check prerequisites, introduction order, cognitive load, and explanation ownership across the Book and Reference; audit major SDK feature coverage when requested. Generated API wiring or Sphinx build failures alone belong to sphinx-docs-review.
---

# Book Reading Review

Review what a reader can understand, decide, or do at each point. OMMX aims to
let transformation and preprocessing techniques accumulate and be reused across
modelers and solving methods through a common problem representation. Keep that
purpose connected to concrete capabilities, with current availability distinct
from future development. Explain it in optimization terms without requiring
compiler knowledge.

OMMX defines problem and candidate semantics independently of a solver. Keep
that common model separate from an Adapter's execution contract and the
guarantees of an individual transformation. Preserving problem structure lets
later processing use its meaning; reconstruction, feasibility, objective values,
and optimality remain distinct guarantees.

This workflow can stand alone for a reader-comprehension question. A change
limited to API generation, directives, or Sphinx build/render/link correctness
belongs to `sphinx-docs-review` and does not require this skill or its ledger.
Apply both only when the request includes both concerns; do not load the other
skill merely because a Book file changed.

## Establish the reader and source

- Resolve the requested PR/revision and read its actual files. A local checkout
  may contain a later stacked PR; do not review that as the requested change.
- Resolve the current editorial direction from the task and accepted design.
  Historical navigation, page roles, and ledger declarations are inputs to
  review, not constraints that override a newer design.
- Identify the reader's goal, starting knowledge, entry link, and useful stopping
  point, including the understanding or decision the page should enable.
  Knowing Python and ordinary optimization does not imply knowing OMMX types,
  retained variables, removed constraints, or Preparation.
- Follow the entry that the navigation promises, including direct chapter or
  Reference entry and alternative routes. Organize the primary navigation by
  reader purpose and the objects they work with; document genres and chapter
  numbers do not establish prerequisites. A declared prerequisite is different
  from a recommended link.
- Separate conceptual knowledge from execution dependencies. Hidden cells still
  require their imports, input data, and prior runtime state.

Notebook execution correctness is covered by the repository's separate tests.
Do not add Notebook reruns to a Book review. Inspect the example and its displayed
outputs for explanatory sufficiency; runtime test failures belong to the test
workflow. Keep documented SDK versions and prerequisites accurate without
treating a Book build as execution evidence.

## Check purpose and capability explanations

For README and overview work, check whether readers can explain what OMMX aims
to provide, why it matters to them, what work they can currently delegate, and
where to go next. A list of APIs or an aspiration alone does not answer those
questions. Keep the modeler, common OMMX processing, and execution backend's
responsibilities understandable without a catalogue of implementation details.

For a capability explanation, identify its use, its place in the overall flow,
and the choices left to the reader. A small example, diagram, or counterexample
should teach the meaning or decision at hand. Retain the conditions needed to
interpret that example correctly; do not require a full implementation recipe
for every feature in the Book.

## Audit feature coverage when requested

For a whole-Book coverage audit, start from the public SDK at the documented
version and the reader tasks it supports, including tasks absent from the Book
or ledger. Group APIs by useful operations rather than requiring a page for
every method. Check the public bindings/exports against their owning
implementation; distinguish released capabilities from development-only APIs.

For each major capability, locate the Book explanation of its existence, use,
and selection criteria, the entry path, and the detailed Reference destination.
An API entry or a release note alone does not explain why to use a capability.
A complete usage example in the linked Reference can supply procedural coverage;
do not require duplicating it in the Book. Inspect examples where they actually
live and record whether coverage is usable, only mentioned, planned, absent, or
deliberately excluded. Distinguish missing implementation of an existing plan
from a proposed expansion of scope.

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
  to the detailed destination and teach only what the current explanation or
  example needs.

## Inspect explanation ownership

For each affected page, identify what the passage enables its reader to
understand or do. Inspect retained passages as well as newly added lines.

Assign ownership by the reader's question, not by a Tutorial/How-to/Guide label:

- README explains the project's purpose, present adoption value, and entry points.
- Book explains the overall model, available capabilities, why and when to use
  them, choices, and result interpretation. Keep small examples and the
  conceptual distinctions needed to understand them.
- API Reference owns detailed calls and usable recipes, exact argument/default
  rules, exhaustive cases, execution order, formulas, preconditions, errors,
  mutation guarantees, and numerical contracts.
- Setup, CLI/operational reference, and format/integration specifications own
  their respective environment, command, authentication, and interoperability
  details. Python API entries cannot substitute for those destinations.

Keep useful standalone tutorials and extended runnable examples discoverable
from the relevant capability. Judge each by its stated purpose; neither a
mandatory tutorial sequence nor a complete recipe in every chapter is required.
Related API operations may need a complete example together rather than scattered
call fragments. Reference should serve both human and AI-assisted implementation.

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

- a missing introduction or poor explanation, which belongs at the responsible
  Book or Reference destination;
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

Link the follow-up from the review record. Keep any documented workaround
accurate and runnable at its owning destination; do not silently redesign the
SDK in a Book PR. An ergonomic follow-up need not block the Book when the
documented workflow is correct, but filing an issue does not excuse an incorrect
example or promise.

## Reconcile changed explanations with the ledger

The Book-owned data is `docs/_meta/reading_paths/manifest.json`, not part of the
skill's instructions. It starts as a proposed design based on an explicitly
recorded revision, not a certification of the current Book.

Keep the ledger focused on meaningful reader outcomes, explanation owners, and
representative direct-entry routes. Reuse valid semantic concept IDs; do not
create a prerequisite node for every API name or require every operation to
appear in a Book route. The existing concept/page/route model and helpers can
serve a new structure without preserving old genre-based IDs or role assignments.
Record project-specific chapters and feature inventories in the ledger, not here.

For each changed reader-facing page affecting prerequisites, introductions, or
navigation, compare the actual prose and Notebook cells with the affected
concept, page, and route contracts. Locate the introduction and first
understanding-dependent use; check their order, scope, and entry knowledge
against the declarations.
This comparison is required, not optional because the analyzer passes. If the
page or concept is untracked, record that coverage gap. Distinguish a current
reading defect from a declaration superseded by the accepted design. Update
affected contracts when the task includes ledger edits; otherwise report the
mismatch and any deferred migration without rewriting them or forcing prose
back into the old structure. A skill-only change does not require a manifest
migration. Query only the affected subset, not the whole Book.
The query helper returns declarations alongside whole-manifest diagnostics,
even when contracts are inconsistent. Use those diagnostics to interpret the
target; successful lookup is not a consistency or readiness verdict.

The helper commands below are optional tools for inspecting the declarations;
they do not replace the required comparison. Select page/concept IDs from the
current manifest; the IDs in these examples are illustrative. If used, run them
from the repository root through its Python environment:

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

For each finding, name the reader/route, the unanswered question or concrete use
and source position, the understanding not yet supplied, and the smallest change
that restores the intended outcome. Distinguish an omitted explanation from
merely unfamiliar spelling. Prefer a local gloss, a reorder, or deferring an
unnecessary detail over making the reader complete another whole chapter.

For an explanation-placement finding, identify the reader's question, the
detail's canonical owner, and the smallest Book explanation to retain. When
reviewing a PR, distinguish newly introduced problems from inherited cleanup
candidates in the affected pages.

Keep these outcomes separate:

- observed text and outputs at the reviewed revision;
- consistency of the proposed learning contracts;
- manual review of purpose, selection guidance, explanatory sufficiency, and
  entry-page cognitive load;
- code execution, version compatibility, and mathematical/API correctness.

Before concluding, leave a concise [current-revision review record](references/schema.md#current-revision-review-record)
in the current chat or an authorized issue/PR discussion. Do not add repository
review-note files unless explicitly requested. Identify the reviewed revision,
paragraphs or Notebook cells, reader routes, and actual introduction/use and
destination evidence. Distinguish checked coverage from unresolved or unreviewed
routes; do not create a whole-Book PASS report for a scoped review.

Recommend or mark Ready only for the reviewed scope and within the task's
authorization, after the affected prose and applicable contracts have been
reconciled, the review record is present, and moved explanations have usable
destinations at that revision. A contract PASS or a warning-free Sphinx build
cannot satisfy those conditions. Keep the
reading review incomplete when an affected route still has a gap.
Build/API-reference checks remain the responsibility of the existing Sphinx
workflow; do not broaden this review into SDK changes or a Book-wide redesign.
