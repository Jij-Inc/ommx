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

- finite assignments and continuous/integer/binary domains whose rational
  interval bounds have explicit `-∞` and `+∞` endpoints;
- sound affine interval evaluation over those domain bounds;
- affine equalities and inequalities, objective value, and optimization sense;
- partial `Instance` transformations with explicit target, encode, and decode;
- directional reduction, relaxation, objective-preservation, and round-trip
  contracts with compatible composition;
- structural OneHot recognition up to an arbitrary nonzero equality scale;
- Indicator augmentation and replacement obligations, exact active
  substitution, and an executable augmentation checker;
- forward Indicator Big-M lowering for arbitrary exact function denotations,
  including the SDK's independent upper/lower side emission and bound-justified
  side omission rules;
- an active-on-one Indicator Big-M `Instance.Transform`: one Indicator
  occurrence is removed, its generated affine rows are appended to the ordinary
  constraints, and the same state space is retained;
- binary-cardinality SOS1 recognition from a complete `≤` constraint with a
  strictly positive scale (the checker rejects an equality with the same affine
  expression);
- executable selector-use/isolation checking over the finite `Instance` AST,
  including domains, every constraint collection, and the objective;
- direct selector-formulation SOS1 theorems for mixed reused/fresh selectors,
  zero-bound link omission, and canonical selector construction;
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

An `Instance.Transform` owns its target Instance and partial state maps.
Reduction and relaxation separately require decode and encode to preserve
feasibility in their respective directions. Objective preservation includes
both objective values and optimization sense. Source and target round trips
are independent because private selector assignments may be noncanonical.
Concrete lowerings keep their independently checkable construction witnesses
outside `Instance.Transform`; the common transform type records only the
resulting target and state maps.

## Modules

| Module | Responsibility |
| --- | --- |
| `OMMXProof.Domain` | Binary/integer/continuous membership, explicitly unbounded interval endpoints, and intrinsically valid nonempty rational bounds |
| `OMMXProof.Function.Affine` | Exact affine algebra and evaluation, substitution, independence, and sound domain-box bounds |
| `OMMXProof.Instance` | Finite Instance syntax and exact denotation |
| `OMMXProof.Instance.Extend` | Left-block embedding of states, expressions, constraints, and Instances into a larger finite space |
| `OMMXProof.Instance.Transform` | Partial state transformations, directional reduction/relaxation and objective-preservation contracts, and independent source/target round trips |
| `OMMXProof.Instance.Transform.IndicatorBigM` | Checkable Indicator Big-M witnesses, target construction, identity state maps, and semantic correctness |
| `OMMXProof.Instance.Transform.SOS1BigM` | Checkable SOS1 Big-M witnesses, target construction, state maps, and semantic correctness |
| `OMMXProof.Linear.EqualityNonnegativeLP` | Equality-form LP with nonnegative variables |
| `OMMXProof.Linear.LessEqualNonnegativeLP` | Less-than-or-equal-form LP with nonnegative variables |
| `OMMXProof.Constraint.Linear` | Normalized affine equality and inequality semantics |
| `OMMXProof.Constraint.OneHot` | OneHot semantics, structural checker, and direct replacement equivalence |
| `OMMXProof.Constraint.Indicator` | Indicator semantics, active substitution, and structural promotion obligations |
| `OMMXProof.Constraint.SOS1` | SOS1 semantics and direct selector-formulation theorems |
| `OMMXProof.Constraint.SOS1.Instance` | Executable selector-isolation checking over the complete Instance AST |
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

- [x] Semantic domains with explicit infinite endpoints and valid rational bounds
- [x] Sound affine bounds, Instance denotation, and Transform preservation relations
- [x] Equality and less-than-or-equal nonnegative LP semantics
- [x] Reduction/relaxation composition and encode/decode round-trip laws
- [x] OneHot and Indicator recovery rules
- [x] Indicator Big-M lowering as a finite `Instance.Transform`
- [x] SOS1 selector formulations with mixed reused/fresh selector layouts
- [x] SOS1 Big-M lowering as a finite `Instance.Transform`
- [x] Executable accept/reject fixtures and counterexamples

The canonical bridge, trace vocabulary, and Rust proof-log producer remain
future integration work and are intentionally unimplemented here.
