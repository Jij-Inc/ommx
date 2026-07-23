# OMMX proof-carrying reduction semantics

This directory implements the independent Lean semantics described in
[issue #1059](https://github.com/Jij-Inc/ommx/issues/1059) for designing
proof-carrying presolve and special-constraint recovery.
The parent runtime workstream is
[issue #1057](https://github.com/Jij-Inc/ommx/issues/1057).

The model is intentionally independent of the OMMX Rust SDK and protobuf
schema. It consumes no OMMX runtime artifact and does not claim to formally
verify the Rust implementation. Future integration work will define and review
the refinement bridge from committed OMMX snapshots and proof traces.

## Scope

The independent formalization defines exact rational semantics for:

- finite assignments, continuous/integer/binary domains, and finite bounds;
- affine equalities and inequalities, objective value, and optimization sense;
- identity-space, directed implication, infeasibility, and projection/lift
  preservation contracts;
- compatible reduction composition with lifts composed in reverse order;
- structural OneHot recognition up to an arbitrary nonzero equality scale;
- Indicator augmentation and replacement obligations, exact active
  substitution, and an executable augmentation checker;
- forward Indicator Big-M lowering for arbitrary exact function denotations,
  including the SDK's independent upper/lower side emission and bound-justified
  side omission rules;
- binary-cardinality SOS1 recognition from a complete `≤` constraint with a
  strictly positive scale (the checker rejects an equality with the same affine
  expression);
- executable selector-use/isolation checking over the finite `Instance` AST,
  including domains, every constraint collection, and the objective;
- mixed reused/fresh selector-gadget SOS1 projection with the SDK's zero-bound
  link omission rules, canonical lift, objective isolation, and the
  counterexample to source-side retraction. The checked isolation witness is
  consumed directly by a `ProjectionPreserves` compression theorem; the
  all-fresh, full-link formulation remains available as the simpler core case.
- an actual SDK-style SOS1 Big-M `Instance.Transform`: one SOS1 occurrence is
  removed, binary members are reused, fresh binary components are appended for
  the other members, and optional link rows plus the cardinality row are added.
  The lowering is both a reduction and relaxation, has source round-trip and
  objective/sense preservation, but deliberately has no target round-trip.

Detector completeness, floating-point tolerance, Rust mutation correctness,
serialization, lifecycle/capability/audit state, and a complete integer proof
system are outside this independent model.

## Formal contract

The independent semantics research contract consists of the input AST,
normalization, denotation, preservation relations, witness schemas, and
executable checker acceptance rules. It is not a stable OMMX wire-format or
public SDK version. A version identifier will be introduced with the canonical
bridge when an external proof trace or decoder needs to select a compatible
contract.

`LinearConstraint.normalize lhs rhs sense` specifies the version-1 rule of
moving the right-hand side to the left. Rows are then represented as
`a · x + c ≤ 0` or `a · x + c = 0`. The semantics use exact `Rat`;
numerical tolerances never create a proof.

Projection preservation requires both feasible directions, objective
preservation in both directions, and the section law
`project (lift y) = y` for feasible reduced assignments. It deliberately does
not require `lift (project x) = x`, because private selector assignments may be
noncanonical and unobservable.

## Modules

| Module | Responsibility |
| --- | --- |
| `OMMXProof.Instance` | Finite Instance syntax and exact denotation |
| `OMMXProof.Instance.Extend` | Left-block embedding of states, expressions, constraints, and Instances into a larger finite space |
| `OMMXProof.Instance.Transform` | Partial state transformations, directional reduction/relaxation contracts, and independent source/target round trips |
| `OMMXProof.Instance.Transform.SOS1BigM` | SDK-style SOS1 Big-M target construction, state maps, and semantic correctness |
| `OMMXProof.Linear.EqualityNonnegativeLP` | Equality-form LP with nonnegative variables |
| `OMMXProof.Linear.LessEqualNonnegativeLP` | Less-than-or-equal-form LP with nonnegative variables |
| `OMMXProof.SemanticProblem` | Syntax-independent optimization semantics, preservation relations, and composition laws |
| `OMMXProof.Constraint.Linear` | Normalized affine equality and inequality semantics |
| `OMMXProof.Constraint.OneHot` | OneHot semantics, structural checker, and replacement theorem |
| `OMMXProof.Constraint.Indicator` | Indicator semantics, active substitution, and forward Big-M semantics |
| `OMMXProof.Constraint.SOS1` | SOS1 semantics and generic selector compression |
| `OMMXProof.Constraint.SOS1.Instance` | Instance-connected selector isolation and SDK-plan compression |
| `OMMXProofTest.Fixtures` | Test-only accepted/rejected fixtures and counterexamples |
| `OMMXProofTest.Acceptance` | `lake test` acceptance target |
| `OMMXProofTest.Trust` | Elaborated-environment audit rejecting project-defined axioms |

## Checks

Lean 4 and Mathlib are pinned together at `v4.31.0`. From the repository root:

```shell
task lean:cache
task lean:check
```

Or from this directory:

```shell
task cache
task check
```

`task check` verifies generated umbrella imports, treats warnings (including
`sorry`) as errors, builds the soundness library, audits its elaborated
environment for project-defined axioms, and elaborates the executable fixtures.
The separate `OMMXProofTest` library uses `native_decide` only to execute small
accept/reject fixtures; those compiler-backed fixture proofs are not imported
by `OMMXProof` and are never premises of its soundness theorems. Printed `#eval`
output is not a correctness gate. The stated production trust base still
includes Lean's standard `propext`, `Classical.choice`, and `Quot.sound` axioms.

## Implemented scope

- [x] Semantic domains, denotation, and preservation relations
- [x] Equality and less-than-or-equal nonnegative LP semantics
- [x] Reduction composition and project/lift laws
- [x] OneHot, Indicator recovery, and Indicator Big-M lowering rules
- [x] SOS1 projection and mixed reused/fresh SDK-plan compression
- [x] SOS1 Big-M lowering as a finite `Instance.Transform`
- [x] Executable accept/reject fixtures and counterexamples

The canonical bridge, trace vocabulary, and Rust proof-log producer remain
future integration work and are intentionally unimplemented here.
