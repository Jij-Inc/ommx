# Learning-contract schema and source evidence

The checker consumes manually maintained JSON. Neither it nor the query helper
calls an LLM, infers knowledge from words, or validates mathematical equivalence.
Run commands from the repository root through the project Python environment.

## Maintained data

`docs/_meta/reading_paths/manifest.json` is the Book-owned source for proposed
concept/page/route contracts. The initial data comes from the 2026-10-26 design
audit of the revision recorded in `baseline.source_revision`. Its status and
fixed source URLs describe that baseline, not the live PR or `main`.

Resolve the actual revision before reviewing prose. If it differs, inspect the
changed passages and record the reviewed evidence separately. Do not change a
revision field to make old annotations look current. Planned owners are not
already-available explanations. The ledger's open review notes are not waived
by a successful contract check.

## Current-revision review record

Keep scoped review evidence in the existing review response or work notes,
separate from the historical baseline and generated analyzer output. Use any
concise format that records:

- **Source:** reviewed commit; for uncommitted work, identify the base commit
  and changed files/diff rather than claiming that the base contains the edits.
- **Reader route:** goal, initial knowledge, entry, visited pages/sections, and
  useful stopping point. Distinguish direct entry from a prerequisite route.
- **Passages:** file plus heading/paragraph or Notebook cell ID/index, locating
  each relevant introduction and first understanding-dependent use. Explain
  briefly why the introduction teaches what the use needs.
- **Reconciliation:** affected concept/page/route IDs and mismatches or missing
  contracts. Report analyzer results separately from body review.
- **Ownership and moves:** what the reader must understand from the Book and
  which exact contract belongs to the API Reference; for a relocation, record
  the source and actual destination passage. Confirm that the entry reaches
  the needed explanation/procedure, not merely another reference. Record a
  missing API explanation as a gap rather than treating a resolving link as
  sufficient.
- **Outcome:** checked scope, unresolved findings, and unreviewed routes. Do
  not extrapolate a scoped success to the rest of the Book.

Do not replace `baseline.source_revision`, silently certify proposed contracts,
or commit generated PASS output as review evidence. When prose, introduction
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
and `scope` describe the actual capability. Aliases are search aids, not evidence
that two capabilities are equivalent. Owners provide detailed explanations;
independent entries may teach the required subset locally.

Each page has a unique `id`, entry `requires`, `deferred`, and ordered `sections`.
The query helper additionally requires a nonempty `title` on every page;
`source_paths` is used when querying optional source evidence.
Each section has an ID unique within that page and these lists:

| Field | Reader effect |
| --- | --- |
| `introduces` | Teach capabilities left to right, checking their prerequisites |
| `uses` | Require capabilities after the section's introductions |
| `mentions` | Record exposure without granting knowledge |

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

Analysis output and rendered reports are derived artifacts, not additional
editable copies of the contracts. Do not commit a stale PASS report alongside
an edited manifest or automatically turn a PASS into a PR readiness decision.

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
