# OMMX Instance semantics and transformation correctness

This directory gives an independent Lean formalization of:

- the mathematical optimization problem denoted by an OMMX `Instance`; and
- the mathematical correctness of transformations from one `Instance` to
  another.

The first goal is to make feasibility, objective values, optimization sense,
domains, and constraints mathematically explicit. The second is to state and
prove exactly what a transformation preserves through its target Instance and
its encode/decode maps.

For now, this formalization is a reference semantics for implementation and
design. It is intentionally independent of the OMMX Rust SDK and protobuf
schema, consumes no SDK artifact, and does not claim to verify the Rust
implementation.

The intended future integration boundary is a transformation-specific
`Witness`: the SDK transforms an Instance and emits a `Witness`, and Lean checks
it against the source and target Instances. Establishing that boundary will
require a versioned contract and a reviewed refinement bridge between SDK data
and this exact semantic model.

This direction is tracked in
[issue #1059](https://github.com/Jij-Inc/ommx/issues/1059); related SDK/runtime
integration is tracked in
[issue #1057](https://github.com/Jij-Inc/ommx/issues/1057).

## Current scope

The current model deliberately focuses on a finite-dimensional, exact-rational
affine fragment. It provides:

- mathematical semantics for states, domains, affine functions, constraints,
  objectives, and optimization sense;
- an `Instance.Transform` contract with explicit target, encode/decode,
  directional preservation, round-trip, and composition laws; and
- representative exact results for special-constraint recognition, promotion
  conditions, and Big-M lowering.

Detailed modules and implemented features are listed below. SDK serialization
and lifecycle, floating-point behavior, Rust mutation correctness, and
completeness of recognition or presolve algorithms remain outside this model.

## Modules

| Module | Responsibility |
| --- | --- |
| `OMMXProof.Domain` | Binary/integer/continuous membership, explicitly unbounded interval endpoints, and intrinsically valid nonempty rational bounds |
| `OMMXProof.Function.Affine` | Exact affine algebra and evaluation, substitution, and sound domain-box bounds |
| `OMMXProof.Instance` | Finite Instance syntax and exact denotation |
| `OMMXProof.Instance.Extend` | Left-block embedding of states, expressions, constraints, and Instances into a larger finite space |
| `OMMXProof.Instance.Transform` | Partial state transformations, directional reduction/relaxation and objective-preservation contracts, and independent source/target round trips |
| `OMMXProof.Instance.Transform.IndicatorBigM` | Checkable Indicator Big-M witnesses, target construction, identity state maps, and semantic correctness |
| `OMMXProof.Instance.Transform.SOS1BigM` | Checkable SOS1 Big-M witnesses, target construction, state maps, and semantic correctness |
| `OMMXProof.Constraint.Linear` | Normalized affine equality and inequality semantics |
| `OMMXProof.Constraint.OneHot` | OneHot semantics, structural checker, and direct replacement equivalence |
| `OMMXProof.Constraint.Indicator` | Indicator semantics, active substitution, and structural promotion obligations |
| `OMMXProof.Constraint.SOS1` | SOS1 semantics and direct selector-formulation theorems |
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
- [x] Reduction/relaxation composition and encode/decode round-trip laws
- [x] OneHot and Indicator recovery rules
- [x] Indicator Big-M lowering as a finite `Instance.Transform`
- [x] SOS1 selector formulations with mixed reused/fresh selector layouts
- [x] SOS1 Big-M lowering as a finite `Instance.Transform`
- [x] Executable accept/reject fixtures and counterexamples

## Planned directions

The packaged `Instance.Transform` examples currently focus on lowering special
constraints into ordinary constraints. The common transformation contract is
not restricted to lowering or to presolve reductions. The intended next
directions are:

- promotion of ordinary constraints to special constraints when a
  transformation-specific `Witness` establishes their semantic equivalence;
- presolve transformations for MILP problems represented by `Instance`;
- more general presolve transformations beyond the current affine MILP
  fragment;
- an infeasibility-certificate framework that standardizes an appropriate
  relaxation, checks a certificate on the standardized problem, and translates
  the certified result back to the source `Instance`; and
- a canonical SDK-to-Lean bridge in which SDK-produced `Witness` values are
  checked against this formal contract.

The versioned `Witness` contract, bridge, and SDK-side `Witness` producer are
future integration work and are intentionally unimplemented here.
