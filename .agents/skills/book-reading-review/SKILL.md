---
name: book-reading-review
description: Review OMMX Book tutorials, guides, and navigation for reader prerequisites, unfamiliar concepts, introduction order, cognitive load, and independent reading paths. Use when planning or changing those learning paths, rather than for generated API wiring or Sphinx build failures alone.
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

Tutorials teach one practical workflow; How-tos complete an independent task;
Guides own concepts and interpretation; API Reference owns exact formulas,
preconditions, errors, and mutation guarantees. Value reconstruction,
feasibility, objective values, and optimality are distinct guarantees.

When moving or removing an explanation, read its actual replacement and follow
the affected incoming entry through to the definition or usable procedure.
A page, heading, anchor, or resolving link is not evidence that the explanation
exists. Follow onward references until the reader reaches the needed content;
do not accept a chain that only sends them back to the removed explanation.
The replacement must be available in the reviewed PR/revision, not only in a
later stacked PR. Report an unmet route rather than recommending Ready while
its replacement is still planned.

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
