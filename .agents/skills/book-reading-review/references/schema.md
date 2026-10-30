# Learning-contract schema and source evidence

The checker consumes manually maintained JSON. Neither it nor the query helper
calls an LLM, infers knowledge from words, or validates mathematical equivalence.
Run commands from the repository root through the project Python environment.

## Maintained data

`docs/_meta/reading_paths/manifest.json` owns the Book-specific concepts,
explanation responsibilities, and representative reader routes. This skill
owns the review method; current-revision review records supply the evidence.
The ledger's recorded baseline, status, and fixed source URLs describe its
design inputs, not the live PR or `main`.

Resolve the actual revision before reviewing prose. If it differs, inspect the
changed passages and record the reviewed evidence separately. Do not change a
revision field to make old annotations look current. Planned owners are not
already-available explanations. The ledger's open review notes are not waived
by a successful contract check.

When a content migration is authorized, record the new design source and target
revision explicitly, reassess the affected contracts, and retain the previous
ledger in Git history rather than maintaining two active copies. A Skill-only
edit does not require or authorize that migration. Report affected mismatches
against the accepted design and actual prose, distinguishing a missing
explanation from a superseded declaration; unrelated historical diagnostics do
not determine readiness of the reviewed change.

## Current-revision review record

Keep scoped review evidence in the existing chat response or, when posting is
authorized, the relevant issue or PR discussion. Do not create tracked review-note
files unless explicitly requested, or commit generated PASS reports. Keep
evidence separate from the design ledger and use any concise format that records:

- **Source:** reviewed commit; for uncommitted work, identify the base commit
  and changed files/diff rather than claiming that the base contains the edits.
- **Reader route:** goal, initial knowledge, entry, visited pages/sections, and
  useful stopping point. Distinguish direct entry from a prerequisite route.
- **Passages:** file plus heading/paragraph or Notebook cell ID/index, locating
  each relevant introduction and first understanding-dependent use. Explain
  briefly why the introduction teaches what the use needs.
- **Reconciliation:** affected concept/page/route IDs and mismatches or missing
  contracts. Report analyzer results separately from body review.
- **Ownership and moves:** what the reader must understand or decide from the
  README or Book, and which details belong to API or operational Reference;
  for a relocation, record the source and actual destination passage. Confirm
  that the entry reaches the needed explanation or complete procedure, including
  examples owned by Reference. Record a missing explanation as a gap rather
  than treating a resolving link as sufficient.
- **Outcome:** checked scope, unresolved findings, and unreviewed routes. Do
  not extrapolate a scoped success to the rest of the Book.
- **Coverage audits only:** documented SDK revision, major capabilities and
  their public API sources, the Book's explanation of purpose and choices,
  and usable destinations for implementation details and examples. Distinguish
  gaps from planned or deliberately excluded work. Do not infer coverage from
  the existing concept catalog alone or require each procedure in the Book.

Notebook execution is owned by the repository's separate tests. The Book review
record describes source, explanations, and displayed outputs; it does not
require rerunning Notebooks or certify their execution from a Book build.

Do not update `baseline.source_revision` merely to certify old annotations or
silently turn proposed contracts into reviewed ones. When prose, introduction
order, or navigation changes after review, revisit only the affected evidence.
A history-only rebase of unchanged content does not require re-executing the
Book. Recording evidence does not itself authorize publishing a PR comment.

## Schema version 1

Required top-level fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | Integer `1` |
| `concepts` | Nonempty list of concept objects |
| `pages` | Nonempty list of page/section contracts |
| `routes` | List of reading routes; may be empty for an isolated contract check |

An empty/missing catalog or an unsupported version is invalid input, not a
successful zero-check run. Book-specific metadata such as `baseline`, `scope`,
and `environment_requirements` is provenance/context, not an extra source of
reader knowledge or a runtime dependency check.

Each concept has a unique nonempty string `id`, a `requires` list of concept
IDs, and an optional `owner` page ID (or `null` for external knowledge). `label`
and `scope` describe the understanding or decision the reader needs. Do not
create a prerequisite concept for each API spelling. Aliases are search aids,
not evidence that two capabilities are equivalent. Owners may be Book or
Reference pages; independent entries may teach the required subset locally.
Separate conceptual understanding from detailed operation contracts when their
uses and explanation owners differ.

Each page has a unique `id`, entry `requires`, `deferred`, and ordered `sections`.
The query helper additionally requires a nonempty `title` on every page;
`source_paths` is used when querying optional source evidence.
Each section has an ID unique within that page and these lists:

| Field | Reader effect |
| --- | --- |
| `introduces` | Teach capabilities left to right, checking their prerequisites |
| `uses` | Require capabilities after the section's introductions |
| `mentions` | Record exposure without granting knowledge |

Page IDs and document-role metadata do not prescribe a navigation hierarchy.
Choose IDs for the page's responsibility and reuse valid concept IDs during a
reorganization. Existing `title`, `scope`, and notes can describe the reader's
question, intended understanding, and detail destinations without changing the
schema. Such metadata is not evaluated as reader knowledge by the checker.

No implicit prerequisite closure is applied to entry knowledge. `parent_page`
identifies an optional or scoped section; it does not import or export knowledge.
Do not merge such a child into its parent's contract just because they share a
physical file. `deferred` prohibits introduction/use here, but permits mention.

Use a separate earlier section to represent use before explanation. A single
section is suitable for a short example and adjacent interpretation, not a
whole chapter whose internal learning order needs inspection.

A route declares `initial` knowledge and ordered `steps`. A step is a page ID or
`{"alternatives": [["A"], ["B"]]}`. Branches start with the same known set and
join by intersection. An empty alternative represents skipping an optional
path; it teaches nothing. The route must supply each visited page's entry
requirements. Invalid pages/concepts must not produce a successful route result.
Use routes for representative goals and direct entries, including a useful
stopping point. Do not turn chapter numbers into prerequisites or require a
route for every operation documented in Reference.

`status`, `contract_status`, and `implementation_verified` do not teach anything.
In particular, `existing` means a physical baseline source exists, not that it
implements the proposed contract. All initial contracts are proposed and their
body implementation remains unverified.

## Query and analysis

```sh
uv run --no-sync python .agents/skills/book-reading-review/scripts/query.py docs/_meta/reading_paths/manifest.json concept variables.defaults
uv run --no-sync python .agents/skills/book-reading-review/scripts/query.py docs/_meta/reading_paths/manifest.json page G8
uv run --no-sync python .agents/skills/book-reading-review/scripts/analyze.py docs/_meta/reading_paths/manifest.json --output /tmp/book-contract-analysis.json
uv run --no-sync python -m unittest discover -s .agents/skills/book-reading-review/scripts -p 'test_*.py' -v
```

The analyzer exits `0` for consistent contracts, `1` for reference/learning-flow
violations, and `2` for malformed input, unsupported schema, or I/O failure.
Inspect findings and individual route results, not just the exit code. Knowledge
reported on a failed route is diagnostic, not a guarantee about its reader.

The query helper returns the requested declaration even when learning contracts
are inconsistent. Its `manifest_diagnostics` contains whole-manifest consistency
and findings; page queries also include `target_page_analysis`. Diagnostics may
refer to unrelated pages and do not prevent inspection of the requested target.
Query exit `0` means the lookup succeeded, not that contracts passed. Exit `2`
indicates malformed input, empty/duplicate concept or page IDs that prevent
unambiguous indexing, an unknown target, incompatible evidence, or I/O failure.
The analyzer remains the command for a whole-manifest consistency verdict.

Analysis output and rendered reports are derived artifacts, not additional
editable copies of the contracts. Do not commit a stale PASS report alongside
an edited manifest or automatically turn a PASS into a PR readiness decision.
The checker does not assess whether adoption reasons are persuasive, a feature's
purpose is clear, or a Reference example is sufficient; inspect the prose and
destinations for those judgments.

## Optional historical evidence

Without extra input, `query.py` reports declared contracts only and marks actual
source evidence unassessed. If a source-audit bundle is supplied, pass its
directory explicitly:

```sh
uv run --no-sync python .agents/skills/book-reading-review/scripts/query.py docs/_meta/reading_paths/manifest.json concept class.membership --evidence-dir /absolute/path/to/audit
```

Supported bundles contain `source_inventory.json`, `observed_appearances.json`,
and `concept_crosswalk.json` from the original audit format. Their source
revision must match each other and `baseline.source_revision`. Revision equality
is a metadata consistency check, not proof that the annotations match current
file bytes. Inspect the fixed revision's text or compare the recorded hashes
before using it as evidence. Do not merge observations from another revision
silently or interpret a missing observation as absence from the prose.

Keep native roles, source positions, and crosswalk relation strength. A broad or
approximate mapping is not an exact capability and does not grant knowledge.
Likewise, an occurrence classified as `mention` or `output_only` is not mastery.
Same-file observations do not automatically belong to a virtual child section.

The fixed-commit audit builder and its code-comment/Setext heading parser are
not included as maintained tools. Use explicit source paths, notebook cells,
and reviewed headings; do not revive that parser to infer page identities.

## Annotation review examples

- An early used-variable explanation may require understanding removed
  constraints or output objectives even when it links to their owner. Check
  whether that owner requires knowledge taught later in the unfinished page.
- Ordinary variable-domain knowledge must not automatically grant knowledge of
  semi-continuous/semi-integer domains. If a default-value rule needs that
  distinction, annotate the narrower prerequisite or reorder the explanation.
- A SampleSet entry can postpone transformed-objective details until after its
  basic evaluation example. Decide whether the early reference merely previews
  the topic or actually asks the reader to use it; do not classify by the word
  alone.
- A homepage's unexplained transformation names can overload an entry even
  when all are correctly marked `mentions`. This requires prose/reader review,
  not another edge added merely to force the checker to fail.
