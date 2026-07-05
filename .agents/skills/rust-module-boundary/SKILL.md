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
keyword that would make the current call site compile. Start from the
operation → representation → API derivation defined as the Mandatory Design
Order in `.agents/skills/domain-responsibility-review/SKILL.md`. That order is
owned by the domain skill; do not restate or fork it here. This skill
translates its outcome into Rust module structure and visibility.

For each operation identified there, state:

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

`pub(crate)` is still an API across owner boundaries. Do not use crate-internal
visibility as the method for preserving invariants. Use it only when a named
owner in another top-level module must call a narrow operation whose effect can
be described without exposing broader mutation authority. If the justification
is only "SDK users cannot call it", make the item private or reshape the module
boundary.

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
   - Use `pub(crate)` only when the item must cross top-level module boundaries in this crate and the allowed effect is narrower than the caller's full semantic operation.

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
   - Prefer names that describe concrete storage effects, such as row
     replacement, lifecycle movement, or table-level conversion. Avoid broad
     names like `rewrite`, `update`, `with`, or `into_parts` unless the owning
     abstraction genuinely exposes that whole operation.

## Mechanical First Pass

Before reading the diff by hand, generate the visibility delta and review that
listing. Eye-scanning a large diff misses added `pub` items; a generated
listing does not.

- Enumerate every new visible item and every new mutable access path across
  all changed Rust sources. OMMX Rust code is not confined to `rust/`; the
  PyO3 binding crate lives under `python/ommx/src`, so match by file type
  rather than hard-coding a directory:
  ```
  git diff main...HEAD -- '*.rs' | rg '^\+.*\bpub\b'
  git diff main...HEAD -- '*.rs' | rg '^\+.*(-> *&mut|&mut self)'
  ```
- When `cargo-public-api` and a nightly toolchain are available, produce the
  public API delta of the SDK crate instead of reconstructing it from the
  diff:
  ```
  cargo public-api -p ommx diff main..HEAD
  ```
  CI already runs cargo-semver-checks, but that only answers "is this a semver
  break". The API delta answers "which public items appeared, changed, or
  disappeared" — every added line is an SDK commitment that needs an owner
  justification.
- Feed each listed item into the Review Flow below: which owner, which
  callers, which invariant.

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
   - For every `pub(crate)` item, ask what invariant the callee can enforce
     locally and what invariant remains owned by the caller. If the callee
     accepts a closure or raw parts that let the caller mix these layers, treat
     it as a boundary leak unless the owner analysis proves otherwise.
   - Ask for evidence, not reassurance: name the validator or constructor that
     enforces the invariant on this path, and the test that fails if it is
     bypassed. If either is missing, that missing enforcement or missing test
     is the finding.

4. Write findings in boundary terms.
   - State which owner boundary is being crossed or which invariant can be bypassed.
   - Then point to the concrete visibility or module structure causing it.
   - Recommend a module restructuring or delegation through the owning abstraction, not only a keyword change.
   - Prefer proposing enforcement — a narrower type, a private module, a
     validator asserted by property tests — over a prose warning; see the
     graduation policy in `.agents/skills/domain-responsibility-review/SKILL.md`.

## Checklist

- Which domain abstraction or subsystem owns this item?
- Which callers genuinely need access?
- Is this an SDK commitment, crate-internal contract, private-module contract, or implementation detail?
- Does each public struct document its owned invariants and intended construction/mutation paths?
- Does the module tree express that boundary without `pub(super)` or `pub(in ...)`?
- If `pub(crate)` is used, is the cross-module reason documented and is the exposed effect narrow enough?
- Can any visible item let callers bypass validation, persistence, or source-of-truth invariants?
- Did you generate the visibility delta (`pub` grep or `cargo public-api diff`)
  instead of relying on eye-scanning the diff?
- For each new visible item, can you point to the validator or constructor that
  protects its invariants and the test that fails on bypass? Either gap is a
  finding on its own.
