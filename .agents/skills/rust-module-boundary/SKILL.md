---
name: rust-module-boundary
description: Use when writing or reviewing Rust module structure, item visibility, public API exposure, or pub/pub(crate)/pub(super) choices in OMMX.
---

# Rust Module Boundary

Use this skill whenever Rust code changes introduce, remove, or review module boundaries or item visibility. This applies during implementation and code review.

## Position In The Workflow

Use `.agents/skills/domain-responsibility-review/SKILL.md` first when a change has domain meaning. This skill is the next step: translate that responsibility model into Rust modules, ownership boundaries, and visibility.

## Access Scope Design Principle

When designing access scope, do not start from the field, method, or visibility
keyword that would make the current call site compile. First return to the
underlying mathematical or domain operation system.

Always keep this order explicit:

1. Mathematical/domain operation.
   - State which operation is being performed on the root domain object.
   - Describe the action independently of the current Rust fields or helper
     methods.
2. Representation and invariants.
   - State which data structures represent the operation's state.
   - State which invariants each structure owns, and which invariants require
     the root object because they cross table or collection boundaries.
3. API shape and visibility.
   - Derive the callable API and its visibility from the operation and
     invariant analysis.
   - Expose only the table-local or collection-local effects required by the
     root operation.

For each operation, state:

- which data the operation must read;
- which data the operation must change;
- which smaller storage components are only implementation targets of that
  root operation;
- which API preserves the invariants without granting broader mutation
  authority.

Derive access scope from that analysis. Grant mutation authority only to the
object that owns the mathematical/domain operation. Lower-level tables or
collections may expose storage primitives only for the specific row-level
effects needed by that owner; they should not expose broad `&mut` access that
lets sibling modules perform part of the root operation without the owner.

## Writing Flow

1. Identify the owner before choosing visibility.
   - Name the domain abstraction or internal subsystem that owns the operation.
   - Restate the mathematical or domain operation first, then identify which
     data it must read or modify.
   - Identify its callers and whether those callers are inside the same module, sibling modules, the crate, or SDK users.
   - State the invariant or API commitment that the boundary must protect.

2. Choose the narrowest visibility that matches the owner boundary.
   - Use plain private items for implementation details inside one module.
   - Use `pub` inside a private module when sibling code inside that private module boundary needs the item; the module privacy still protects it from the crate API.
   - Use public crate API only when the item is an intentional SDK commitment.
   - Use `pub(crate)` only when the item must cross top-level module boundaries in this crate.

3. Document public owner invariants.
   - For each public Rust struct, put the invariants it owns in the struct-level Rustdoc when possible.
   - Name the constructor, parser, builder, or owning host API that preserves those invariants.
   - If public fields or public mutation methods exist, the docs should make clear which values callers may set directly and which owner-level invariants still must hold.

4. Avoid visibility as a shortcut.
   - Do not use `pub(super)` or `pub(in ...)` to patch around an awkward module hierarchy.
   - First reconsider whether the module tree should express the ownership boundary more directly.
   - Do not expose raw registry, descriptor, config, or persistence plumbing just to make a caller convenient.

5. Document crate-wide visibility.
   - If `pub(crate)` is necessary, add a short comment or documentation explaining why the item must cross top-level module boundaries.
   - The reason should name the owner boundary or cross-module contract, not just "used elsewhere".

## Review Flow

1. Check that visibility follows the domain owner.
   - For each visible item, ask which domain abstraction owns it and which callers should be allowed to use it.
   - If the item crosses an owner boundary, verify that the boundary is intentional.

2. Check for hidden API commitments.
   - Public Rust items are SDK commitments unless they are protected by a private module boundary.
   - Raw OCI descriptors, registry handles, config internals, and persistence helpers should not leak into public APIs unless the low-level API is explicitly intended.
   - Public structs should document the invariants they expect callers and owner APIs to preserve; missing Rustdoc is a review risk when the struct has public fields or construction paths.

3. Check for invariant bypass.
   - A caller should not be able to mutate or construct state in a way that skips the owning abstraction's validation.
   - Load, restore, dynamic, sealed, and Python binding paths should route through the same owner boundary when they share invariants.

4. Write findings in boundary terms.
   - State which owner boundary is being crossed or which invariant can be bypassed.
   - Then point to the concrete visibility or module structure causing it.
   - Recommend a module restructuring or delegation through the owning abstraction, not only a keyword change.

## Checklist

- Which domain abstraction or subsystem owns this item?
- Which callers genuinely need access?
- Is this an SDK commitment, crate-internal contract, private-module contract, or implementation detail?
- Does each public struct document its owned invariants and intended construction/mutation paths?
- Does the module tree express that boundary without `pub(super)` or `pub(in ...)`?
- If `pub(crate)` is used, is the cross-module reason documented?
- Can any visible item let callers bypass validation, persistence, or source-of-truth invariants?
