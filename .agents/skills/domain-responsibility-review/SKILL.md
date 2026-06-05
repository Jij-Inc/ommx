---
name: domain-responsibility-review
description: Use when reviewing OMMX code, PR diffs, or proposed fixes to analyze domain semantics, invariants, and responsibility boundaries before writing findings or changing code.
---

# Domain Responsibility Review

Use this skill at the start of every review. The goal is to review code shape through OMMX domain semantics before judging implementation details.

## Review Flow

1. Restate the global domain semantics.
   - Describe what the changed code means in OMMX terms.
   - Name the domain objects and boundaries involved, such as Artifact, Local Registry, Experiment, Run, Solve, Attachment, Config, Descriptor, Instance, Solution, Adapter, or Trace.
   - Identify the intended source of truth for each domain fact.

2. Identify domain invariants.
   - For each domain object or boundary, state the invariants it must preserve.
   - Check whether those invariants are explicit in code, types, config schema, constructors, validation functions, documentation, or tests.
   - If an invariant is only implicit, treat that as a review risk unless the surrounding code makes it unavoidable.

3. Map operations to owners.
   - For every read, write, conversion, validation, persistence, recovery, mutation, or user-facing API exposure, identify the domain owner responsible for that operation.
   - A free function, associated function, trait method, module boundary, or public API is appropriate only when it matches the domain owner.
   - Do not accept a mechanical move into an associated function as a fix. The code should route through the abstraction that owns the domain responsibility.

4. Verify invariant preservation.
   - Look for operations that can create invalid states, bypass validation, duplicate the source of truth, expose internal descriptors, or let callers mutate state outside the owner boundary.
   - Check load/restore paths as carefully as write paths; persisted config and runtime state must preserve the same invariants.
   - Check dynamic and sealed views against the same domain model.
   - Check lock scopes against domain ownership: keep mutexes only around the shared state protected by that owner, and move slow I/O, registry writes, or storage writes outside the lock when the final owner mutation can still enforce the invariant.

5. Write findings in domain terms.
   - Lead each finding with the broken responsibility or invariant.
   - Then point to the concrete code path and explain how the operation can violate that domain rule.
   - Proposed fixes should name the owning abstraction and route the operation through it.

## Review Checklist

- What is the global domain meaning of this change?
- Which object owns each source of truth?
- Which invariants are required by each domain object or boundary?
- Are those invariants explicitly represented or validated?
- Which operations can bypass the owner or invalidate the invariant?
- Are mutex or lock scopes limited to the protected domain state, with slow I/O or persistence kept outside when possible?
- Do sealed, dynamic, persisted, and Python-facing paths preserve the same model?
