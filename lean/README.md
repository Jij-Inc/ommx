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
- exact Farkas implication and infeasibility checking;
- activity-bound implication, implied equality, exact stored-bound replacement
  with a genuine-tightening check, and non-circular redundant-row removal;
- identity-space, directed implication, infeasibility, and projection/lift
  preservation contracts;
- compatible reduction composition with lifts composed in reverse order;
- structural OneHot recognition up to an arbitrary nonzero equality scale;
- Indicator augmentation and replacement with exact active substitution and a
  surviving-system-only inactive proof; equality replacement carries two
  independent inactive-side witnesses over that same surviving system;
- forward Indicator Big-M lowering for arbitrary exact function denotations,
  including the SDK's independent upper/lower side emission and bound-justified
  side omission rules;
- binary-cardinality SOS1 recognition from a complete `≤` constraint with a
  strictly positive scale (the checker rejects an equality with the same affine
  expression);
- executable selector-use/isolation checking over the finite `CoreModel` AST,
  including domains, linear/special constraints, and the objective;
- mixed reused/fresh selector-gadget SOS1 projection with the SDK's zero-bound
  link omission rules, canonical lift, objective isolation, and the
  counterexample to source-side retraction. The checked isolation witness is
  consumed directly by a `ProjectionPreserves` compression theorem; the
  all-fresh, full-link formulation remains available as the simpler core case.

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
`a · x + c ≤ 0` or `a · x + c = 0`. For a Farkas
implication, inequality multipliers must be nonnegative, equality multipliers
are free, coefficients must cancel exactly, and scalar slack is permitted in
the sound direction. The checker uses exact `Rat`; numerical tolerances never
create a proof.

Projection preservation requires both feasible directions, objective
preservation in both directions, and the section law
`project (lift y) = y` for feasible reduced assignments. It deliberately does
not require `lift (project x) = x`, because private selector assignments may be
noncanonical and unobservable.

## Modules

| Module | Responsibility |
| --- | --- |
| `OMMXProof.Core` | Input AST and exact denotation |
| `OMMXProof.Linear.Farkas` | Executable linear certificate checkers and soundness theorems |
| `OMMXProof.Reduction` | Preservation relations and composition laws |
| `OMMXProof.Special.OneHot` | Structural OneHot checker and replacement theorem |
| `OMMXProof.Special.Indicator` | Active substitution, inactive proof, augment/replace, and forward Big-M semantics |
| `OMMXProof.Special.SOS1` | Binary cardinality, selector-use isolation, and full SDK-plan compression |
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
- [x] Exact linear/Farkas checker and soundness
- [x] Reduction composition and project/lift laws
- [x] OneHot, Indicator recovery, and Indicator Big-M lowering rules
- [x] SOS1 projection and mixed reused/fresh SDK-plan compression
- [x] Executable accept/reject fixtures and counterexamples

The canonical bridge, trace vocabulary, and Rust proof-log producer remain
future integration work and are intentionally unimplemented here.
