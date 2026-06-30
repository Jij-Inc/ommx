---
name: domain-responsibility-review
description: Use when reviewing OMMX code, PR diffs, or proposed fixes to analyze domain semantics, invariants, and responsibility boundaries before writing findings or changing code.
---

# Domain Responsibility Review

Use this skill at the start of every OMMX review. The goal is to review code shape through OMMX domain semantics before judging implementation details.

Treat this as a review pre-pass, not as the required final response order. When writing a code review, still lead with actionable findings; use the domain model to decide which findings matter and how to explain them.

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
   - A free function, associated function, trait method, module boundary, or public API is appropriate only when it matches the domain owner.
   - Do not accept a mechanical move into an associated function as a fix. The code should route through the abstraction that owns the domain responsibility.

4. Verify invariant preservation.
   - Look for operations that can create invalid states, bypass validation, duplicate the source of truth, expose internal descriptors, or let callers mutate state outside the owner boundary.
   - Check every construction layer that can bypass another layer: builders,
     parsers, unchecked constructors, restore/projection helpers, per-sample
     extraction, and Python wrappers. If a lower-level collection cannot
     validate a semantic invariant alone, verify that every host-level entry
     point validates it before exposing the object.
   - Check load/restore paths as carefully as write paths; persisted config and runtime state must preserve the same invariants.
   - Check dynamic and sealed views against the same domain model.
   - Check lock scopes against domain ownership: keep mutexes only around the shared state protected by that owner, and move slow I/O, registry writes, or storage writes outside the lock when the final owner mutation can still enforce the invariant.

5. Write findings in domain terms.
   - Lead each finding with the broken responsibility or invariant.
   - Then point to the concrete code path and explain how the operation can violate that domain rule.
   - Proposed fixes should name the owning abstraction and route the operation through it.
   - If the task is addressing review feedback rather than writing a review, keep the workflow self-contained: read the exact comment and surrounding diff, reconstruct the reviewer concern, search for sibling defects, and fix the responsibility boundary rather than only the commented line.

## Recurring OMMX Review Checks

- Check every root object and constraint family affected by a change: `Instance`, `ParametricInstance`, `Solution`, `SampleSet`, regular constraints, indicator, one-hot, and SOS1. A fix for only the regular path is suspect when sidecars, annotations, parsing, serialization, or statistics are involved.
- For parse/serialize/projection changes, verify round-trips preserve one source of truth: prune or transfer sidecars when absorbing constraints, reject or filter reserved annotation keys at protobuf boundaries, and make projected counts include special constraint families.
- For fallible mutation paths, check atomicity before side effects. Build fallible derived values before inserting constraints, clear stale cached outputs before retryable operations, and validate reserved IDs before registry or storage writes.
- For numeric consistency checks, handle boundary and invalid values explicitly. Absolute tolerances are inclusive unless the API documents otherwise, and NaN/Inf must not pass through `(a - b).abs() > atol` style comparisons silently.
- For public API changes, check all user-facing surfaces together: Rust docs, Python docs, migration guides, stubs, examples, DataFrame flags, docstrings, and Python magic-method return contracts.
- For public Rust structs, check whether the struct-level Rustdoc states the invariants that the type owns: valid IDs, active/removed disjointness, sidecar key coverage, non-empty sets, finite/non-zero numeric values, reserved annotation namespaces, or host-level serialization requirements. If callers can construct or mutate the struct, the docs should name the intended constructors or owner APIs that preserve those invariants.
- For table/collection refactors, distinguish table-level invariants from
  host-level invariants. A table may own row IDs and label/context sidecars,
  but only the enclosing `Instance`, `Solution`, or `SampleSet` can prove that
  row payloads reference known decision variables, parameters, samples, or
  constraints. Review both the lower-level table API and every top-level
  builder/parser/unchecked path.
- For `LogicalMemoryProfile`, prefer `#[derive(LogicalMemoryProfile)]` for
  composite structs. Hand-written impls on composite types are a review risk
  because new fields can be forgotten; reserve manual impls for leaf-like
  wrappers, enums, foreign/generic helper types that cannot derive cleanly, or
  cases that intentionally collapse representation.
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
