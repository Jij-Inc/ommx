---
name: domain-responsibility-review
description: Use when reviewing OMMX code, PR diffs, or proposed fixes to analyze domain semantics, invariants, and responsibility boundaries before writing findings or changing code.
---

# Domain Responsibility Review

Use this skill at the start of every OMMX review. The goal is to review code shape through OMMX domain semantics before judging implementation details.

Treat this as a review pre-pass, not as the required final response order. When writing a code review, still lead with actionable findings; use the domain model to decide which findings matter and how to explain them.

## Mandatory Design Order

For any change that affects mutation authority, table/collection APIs, access
scope, or invariant ownership, keep this order explicit throughout the work:

1. Name the mathematical or domain operation that must be performed.
   - State the root object the operation acts on, such as `Instance`,
     `ParametricInstance`, `Solution`, or `SampleSet`.
   - Describe the algebraic action before discussing fields, methods, or
     visibility.
2. Describe the data structures and invariants that represent that operation.
   - Identify which rows, sidecars, lifecycle states, labels, or provenance
     stores represent the domain state.
   - Separate invariants owned by lower-level tables/collections from
     invariants that only the root object can know.
3. Derive the API shape from the first two steps.
   - Expose only the operations needed to realize the mathematical action while
     preserving the stated invariants.
   - If the API design is not a consequence of the operation and representation
     analysis, stop and redo the analysis.

## Review Flow

1. Restate the global domain semantics.
   - Describe what the changed code means in OMMX terms.
   - Name the domain objects and boundaries involved, such as Artifact, Local Registry, Experiment, Run, Solve, Attachment, Config, Descriptor, Instance, Solution, Adapter, or Trace.
   - Identify the intended source of truth for each domain fact.

2. Identify domain invariants.
   - For each domain object or boundary, state the invariants it must preserve.
   - Separate invariants by the layer that can actually know and enforce them.
     A collection/table can usually own key and sidecar consistency, while a
     top-level object such as `Instance`, `Solution`, or `SampleSet` must own
     cross-table semantic invariants such as "referenced variable IDs exist".
   - Check whether those invariants are explicit in code, types, config schema, constructors, validation functions, documentation, or tests.
   - If an invariant is only implicit, treat that as a review risk unless the surrounding code makes it unavoidable.

3. Map operations to owners.
   - For every read, write, conversion, validation, persistence, recovery, mutation, or user-facing API exposure, identify the domain owner responsible for that operation.
   - When the change affects access scope or mutation authority, first describe the underlying mathematical or domain operation on the root object. Then list which data that operation must read and which data it must change.
   - Treat lower-level tables and collections as implementation targets unless the operation is genuinely local to that table or collection.
   - A free function, associated function, trait method, module boundary, or public API is appropriate only when it matches the domain owner.
   - Do not accept a mechanical move into an associated function as a fix. The code should route through the abstraction that owns the domain responsibility.

4. Verify invariant preservation.
   - Look for operations that can create invalid states, bypass validation, duplicate the source of truth, expose internal descriptors, or let callers mutate state outside the owner boundary.
   - Check every construction layer that can bypass another layer: builders,
     parsers, unchecked constructors, restore/projection helpers, per-sample
     extraction, and Python wrappers. If a lower-level collection cannot
     validate a semantic invariant alone, verify that every host-level entry
     point validates it before exposing the object.
   - When rust-analyzer is available, answer this by semantic query instead of
     enumeration by reading: run `incomingCalls` on the owner-level validator
     and compare its caller set against the construction and mutation paths
     the change adds or touches. A construction path missing from the
     validator's callers is a bypass finding.
   - Check load/restore paths as carefully as write paths; persisted config and runtime state must preserve the same invariants.
   - Check dynamic and sealed views against the same domain model.
   - Check lock scopes against domain ownership: keep mutexes only around the shared state protected by that owner, and move slow I/O, registry writes, or storage writes outside the lock when the final owner mutation can still enforce the invariant.

5. Write findings in domain terms.
   - Lead each finding with the broken responsibility or invariant.
   - Then point to the concrete code path and explain how the operation can violate that domain rule.
   - Proposed fixes should name the owning abstraction and route the operation through it.
   - If the task is addressing review feedback rather than writing a review, keep the workflow self-contained: read the exact comment and surrounding diff, reconstruct the reviewer concern, search for sibling defects, and fix the responsibility boundary rather than only the commented line.

## Evidence Requirements

Do not accept prose reasoning — yours or the author's — as proof that an
invariant holds. For every invariant the change relies on, weakens, or moves,
require two pointers:

1. Enforcing code: the constructor, parser, setter, or validation function
   that makes the invariant hold on every construction and mutation path.
2. A test that fails if the invariant breaks: a unit test, a proptest
   property, or a round-trip test.

If either pointer cannot be named, that gap is itself a finding, usually more
important than the local code shape.

Prefer evidence that scales past reviewer enumeration:

- Cross-table semantic invariants (referenced IDs exist, active/removed
  disjointness, sidecar key coverage) belong in owner-level validation helpers
  that `Arbitrary`-based proptests assert as postconditions. Then new builders,
  parsers, unchecked constructors, and restore paths are covered automatically
  instead of by listing them during review.
- Parse/serialize/projection changes must come with a round-trip property test
  built on the existing `Arbitrary` implementations. A diff that "looks
  symmetric" is not evidence; if the round-trip proptest is missing, that is
  the finding.
- When a confirmed finding could have been prevented by a narrower type, a
  private module, or a typestate (as the Stage pattern already does for
  lifecycle), propose that enforcement as the fix, not a prose warning.

## Table And Collection Ownership Checks

- Classify each changed operation as either a root-level semantic operation or
  a lower-level storage effect. Semantic operations such as parameter
  materialization, substitution, propagation, restore normalization, penalty
  conversion, slack conversion, binary-power reduction, feasibility derivation,
  and solution/sample projection belong to `Instance`, `ParametricInstance`,
  `Solution`, or `SampleSet`, not to the row collection.
- Let tables and collections expose only storage effects they can validate
  locally: preserving row identity, keeping active/removed membership
  disjoint, replacing known rows, moving active rows to removed with a reason,
  restoring a host-normalized row, inserting a row with its sidecar, or
  materializing rows together with owned labels/context. A callback that lets a
  caller compute semantic changes inside the collection is a review risk.
- For v1/protobuf conversion, define `From`/`Into` at the smallest owner that
  has complete information. Use table/collection conversions when rows and
  labels/context/removed reasons are owned together; use the root object when
  conversion needs cross-table facts. Do not add row-level conversions that
  default labels, contexts, removed reasons, fixed values, or sample facts for a
  later layer to overwrite.
- Treat default-fill-then-overlay conversion as a sign that the conversion
  boundary is wrong. Move the conversion to the layer that already has the
  missing information.
- Check every stage and family touched by the change: created, removed,
  evaluated, sampled, regular constraints, indicator constraints, one-hot, and
  SOS1. A boundary fix that preserves sidecars for only one family or lifecycle
  can still leave a parallel escape hatch.

## Recurring OMMX Review Checks

This list is a mechanization backlog, not a permanent checklist. Each entry
exists because reviews caught the same defect class more than once. When you
confirm a recurring finding, propose the enforcement that retires the entry — a
type that makes the state unrepresentable, a lint that rejects the pattern, or
a property test that fails on regression — instead of adding more prose here.
A new entry must name its intended graduation target.

- Check every root object and constraint family affected by a change: `Instance`, `ParametricInstance`, `Solution`, `SampleSet`, regular constraints, indicator, one-hot, and SOS1. A fix for only the regular path is suspect when sidecars, annotations, parsing, serialization, or statistics are involved. When rust-analyzer is available, enumerate the families by running `goToImplementation` on the `ConstraintType` trait instead of recalling them, so the list stays correct when families are added.
- For parse/serialize/projection changes, verify round-trips preserve one source of truth: prune or transfer sidecars when absorbing constraints, reject or filter reserved annotation keys at protobuf boundaries, and make projected counts include special constraint families.
- For fallible mutation paths, check atomicity before side effects. Build fallible derived values before inserting constraints, clear stale cached outputs before retryable operations, and validate reserved IDs before registry or storage writes.
- For numeric consistency checks, handle boundary and invalid values explicitly. Absolute tolerances are inclusive unless the API documents otherwise, and NaN/Inf must not pass through `(a - b).abs() > atol` style comparisons silently. Graduation target: a shared tolerance-comparison helper plus a clippy `disallowed-methods` rule against raw tolerance comparisons.
- For public API changes, check all user-facing surfaces together: Rust docs, Python docs, migration guides, stubs, examples, DataFrame flags, docstrings, and Python magic-method return contracts.
- For public Rust structs, check whether the struct-level Rustdoc states the invariants that the type owns: valid IDs, active/removed disjointness, sidecar key coverage, non-empty sets, finite/non-zero numeric values, reserved annotation namespaces, or host-level serialization requirements. If callers can construct or mutate the struct, the docs should name the intended constructors or owner APIs that preserve those invariants. Graduation target: enable `#![warn(missing_docs)]` on the crate so presence is machine-checked; invariant content remains a review concern.
- For `LogicalMemoryProfile`, prefer `#[derive(LogicalMemoryProfile)]` for
  local Rust data types, including newtypes, tuple structs, and fieldless enums.
  Hand-written impls on ordinary data shapes are a review risk because new
  fields can be forgotten or collapsed. If a type cannot derive cleanly, first
  add the missing standard-library impl or extend the derive macro rather than
  treating the type as an exception. Reserve manual impls for semantically
  special profiling shapes such as foreign/generated types, data-carrying enums,
  stage-specific owner wrappers, or shared ownership types like `Rc`/`Arc`.
  Graduation target: a lint or test that rejects hand-written impls on
  ordinary data shapes.
- For new builder/setter/attachment APIs, add focused tests for both preservation and rejection paths, such as sidecar round-trips and orphan-ID validation.
- For derived analysis or table-building code, avoid recomputing whole-instance partitions inside per-variable or per-row loops; compute the owner-side role/set partition once when the operation needs it repeatedly.

## Review Checklist

- What is the global domain meaning of this change?
- Which object owns each source of truth?
- Which invariants are required by each domain object or boundary, and at what
  layer are they enforceable?
- Are those invariants explicitly represented or validated?
- Are lower-level collection invariants and top-level semantic invariants both
  documented and tested?
- Which operations can bypass the owner or invalidate the invariant?
- Are mutex or lock scopes limited to the protected domain state, with slow I/O or persistence kept outside when possible?
- Do sealed, dynamic, persisted, and Python-facing paths preserve the same model?
- For each invariant, which code enforces it and which test fails when it
  breaks? If either cannot be named, is that reported as a finding?
- If a finding recurs across reviews, which type, lint, or property test would
  retire it?
