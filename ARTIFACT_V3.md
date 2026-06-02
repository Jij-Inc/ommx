# OMMX Experiment / Artifact v3 remaining design

This is a temporary design note for work that is not yet part of the public
API contract. Implemented behavior must live in the normal documentation:

- Python API Reference: `ommx.experiment`, `ommx.artifact`, `ommx.tracing`
- Rust API docs: `ommx::experiment`, `ommx::artifact::local_registry`
- User guides:
  - `docs/en/user_guide/experiment.md`
  - `docs/ja/user_guide/experiment.md`
  - `docs/en/user_guide/tracing.ipynb`
  - `docs/ja/user_guide/tracing.ipynb`
- Tutorials:
  - `docs/en/tutorial/experiment_management.md`
  - `docs/ja/tutorial/experiment_management.md`

Do not keep implemented API specifications here. This file should describe
open design boundaries only.

## Terms

| Term | Meaning |
|---|---|
| Descriptor | OCI descriptor. It claims digest / size / media type / annotations, but does not prove that bytes exist in an OMMX Local Registry. |
| StoredDescriptor | OMMX type proving that the descriptor's bytes exist in a specific Local Registry BlobStore. |
| Unsealed | Mutable state whose component blobs may already be stored, but whose root manifest has not been written. |
| SealedArtifact | State whose root manifest blob exists and whose root descriptor identifies the complete artifact. |
| Published | State where the sealed root descriptor is associated with a Local Registry ref. Publishing updates name resolution; it does not store payload bytes. |
| Draft | User-facing mutable object such as `ArtifactDraft` or `Experiment`. This is an API lifecycle term, not a storage state. |
| LocalArtifact | Immutable view of a sealed / published artifact. It has no API for reopening the same artifact as mutable state. |

## Documentation Routing

This file should not enumerate implemented API semantics. When implemented
behavior needs more explanation, update the API Reference, rustdoc, user guide,
tutorial, or release note instead of this file.

## Remaining Design

### Checkpoint Discovery And Retention

Implemented checkpoint recovery starts from the original Experiment image name.
The public API intentionally does not expose checkpoint image names or
checkpoint Artifact handles as normal user handles.

Remaining questions:

- Should there be a checkpoint discovery command or inspector API?
- What retention policy should prune old checkpoint refs?
- Should retention be age-based, count-based, tied to successful commits, or
  controlled by explicit user commands?
- How should checkpoint retention interact with Local Registry GC?

### Active Run Journal

The current checkpoint model recovers the latest closed Run. If a process is
killed while a Run is still open, payload blobs written by that open Run may
exist in the BlobStore but are not associated with recoverable Run state.

Remaining questions:

- Should OMMX record journal metadata for open Run state?
- What is the minimal durable metadata needed to associate open-run writes with
  a later recovery session?
- How should this journal be cleaned up after a successful Run close or
  Experiment commit?

### Lineage And Run Deletion

`Experiment.fork` records the parent manifest as OCI `subject`; the basic
child creation semantics are implemented. Higher-level lineage inspection and
projection APIs are still open.

Remaining questions:

- `parent()` / `history()` API shape
- `diff(other)` semantics between Experiments
- Run deletion as a child manifest that omits a run, rather than mutating the
  parent Artifact
- Retention policy for parent lineage and its effect on GC
- Whether remote registry referrers are reliable enough for child listing

### Adapter Execution And Diagnostics

`Run.log_solve` is implemented for SolverAdapters. Sampler-oriented execution
and structured diagnostics remain design work.

Remaining questions:

- `log_sample` shape for SampleSet-oriented adapters
- Solve-scoped metadata for backend solver name, backend status, elapsed time,
  and solver version
- `DiagnosticsSink` / `DiagnosticCollector` / `DiagnosticEntry` protocol
- Adapter capability detection for diagnostics support
- Media types and attachment naming for native logs, termination reports, and
  gap timelines
- OTel events should reference diagnostic payloads, not embed the payload body

### Run Attributes And Environment

Run status is implemented. Environment capture is not.

Remaining questions:

- Whether to store OS, Python package versions, solver backend versions, and
  hardware information
- Whether environment data should be Run config, aggregate payload, or
  attachment
- How much should be captured automatically versus explicitly by the user

### OpenTelemetry Schema And Renderers

Trace capture and basic renderers are implemented. A stable span/event schema
for Experiment, Artifact, and solver operations is still open.

Remaining questions:

- Span hierarchy for Experiment / Run / solver / Artifact build / load / push
- Event names such as `ommx.attachment.added`, `ommx.solve.recorded`, and
  `ommx.run.parameter.recorded`
- Tests that default Experiment / Artifact operations do not install a global
  `TracerProvider`
- Live / streaming renderer policy

### GC Extensions

Local Registry GC is implemented for the SQLite-backed registry. Remaining GC
work is outside the first local sweep.

Remaining questions:

- Stale checkpoint ref retention and pruning policy
- Archive / OCI Image Layout reachability analysis
- Remote registry dry-run reporting and deletion capability detection
- How explicit protected digests should surface, if at all, in public APIs
- How lineage retention policy should interact with subject-chain traversal

### Legacy MINTO Import

MINTO-compatible APIs are not planned as the main OMMX API. If migration support
is needed, decide whether it belongs in OMMX core or in a separate migration
tool.

Remaining questions:

- Compatibility loader for `org.minto.*` annotations
- Mapping from MINTO archive structure into OMMX Experiment attachments, Runs,
  Solves, and parameters
- How much lossy migration is acceptable

## Summary Table

| Area | Remaining work |
|---|---|
| Checkpoint retention | Discovery, inspection, pruning policy |
| Active Run journal | Durable metadata for unclosed Run recovery |
| Lineage | `parent`, `history`, `diff`, run deletion projection, retention |
| Sampler execution | `log_sample` and SampleSet-oriented records |
| Diagnostics | Sink protocol, media types, adapter support detection |
| Run environment | OS / package / solver version schema |
| OTel schema | Stable spans, events, and live renderer policy |
| GC extensions | Stale checkpoint pruning, archive / remote registry handling |
| Legacy MINTO import | Compatibility loader or external migration tool |
