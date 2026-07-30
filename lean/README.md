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

The current model focuses on exact-rational affine optimization. It provides:

- mathematical semantics for states, domains, affine functions, constraints,
  objectives, and optimization sense;
- an `Instance.Transform` contract with explicit target, encode/decode,
  directional preservation, round-trip, and composition laws; and
- partial Indicator and SOS1 Big-M lowering that validates a `Plan` before
  constructing an `Instance.Transform`; and
- conservative SOS1 Big-M promotion that validates an untrusted `Witness`
  before establishing transformation correctness.

Detailed modules and implemented features are listed below. SDK serialization
and lifecycle, floating-point behavior, Rust mutation correctness, and
completeness of recognition or presolve algorithms remain outside this model.

## Modules

| Module | Responsibility |
| --- | --- |
| `OMMXProof.State` | Exact rational assignments over packed `Fin n` decision-variable indices |
| `OMMXProof.Domain` | Binary/integer/continuous membership, explicitly unbounded interval endpoints, and intrinsically valid nonempty rational bounds |
| `OMMXProof.Function.Affine` | Exact affine algebra, evaluation, and sound domain-box bounds |
| `OMMXProof.Constraint.Linear` | Normalized affine equality and inequality semantics |
| `OMMXProof.Constraint.OneHot` | OneHot constraint syntax and exact satisfaction semantics |
| `OMMXProof.Constraint.SOS1` | SOS1 constraint syntax and exact satisfaction semantics |
| `OMMXProof.Constraint.Indicator` | Indicator polarity, constraint syntax, and exact satisfaction semantics |
| `OMMXProof.Instance` | Finite Instance syntax and exact denotation |
| `OMMXProof.Instance.Extend` | Left-block embedding of states, expressions, constraints, and Instances into a larger finite space |
| `OMMXProof.Instance.Transform.Basic` | Partial state transformations, directional reduction/relaxation and objective-preservation contracts, and independent source/target round trips |
| `OMMXProof.Instance.Transform.IndicatorBigM.Formulation` | Pointwise Big-M sides and their exact relation to active-on-one Indicator semantics |
| `OMMXProof.Instance.Transform.IndicatorBigM.Plan` | Indicator selection, affine body-bound computation, and validation of supported triggers, polarity, and finite endpoints |
| `OMMXProof.Instance.Transform.IndicatorBigM.Target` | Generated linear rows, target Instance construction, and feasibility equivalence |
| `OMMXProof.Instance.Transform.IndicatorBigM.Basic` | Partial lowering with identity state maps and transformation correctness |
| `OMMXProof.Instance.Transform.IndicatorBigM` | Entrypoint for the complete Indicator Big-M lowering |
| `OMMXProof.Instance.Transform.SOS1BigM.Formulation` | Mixed reused/fresh selector formulation and its exact relation to SOS1 semantics |
| `OMMXProof.Instance.Transform.SOS1BigM.Lowering.Plan` | SOS1 selection, reused/fresh member partitioning, and finite-bound validation |
| `OMMXProof.Instance.Transform.SOS1BigM.Lowering.Target` | Big-M links, selector cardinality, target Instance construction, and feasibility equivalence |
| `OMMXProof.Instance.Transform.SOS1BigM.Lowering` | Partial lowering, canonical encoding, source projection, and transformation correctness |
| `OMMXProof.Instance.Transform.SOS1BigM.Promotion.Witness` | Untrusted retained/fresh suffix layout supplied to the promotion checker |
| `OMMXProof.Instance.Transform.SOS1BigM.Promotion.Target` | Conservative standard-form validation, promoted target construction, and feasibility equivalence |
| `OMMXProof.Instance.Transform.SOS1BigM.Promotion` | Total witnessed promotion plus correctness theorems under validated sufficient conditions |
| `OMMXProof.Instance.Transform.SOS1BigM` | Umbrella for both directions of the SOS1 Big-M formulation |
| `OMMXProof.Instance.Transform` | Entrypoint for common transformation semantics and all concrete Instance transformations |

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
- [x] Partial Indicator Big-M lowering with validated affine body bounds and identity state maps
- [x] Partial SOS1 Big-M lowering with mixed reused/fresh selector layouts
- [x] Conservative SOS1 Big-M promotion from a standard selector/link suffix
- [x] Executable accept/reject fixtures and counterexamples

## Planned directions

The packaged `Instance.Transform` examples include lowering Indicator and SOS1
constraints into regular linear constraints and promoting one standard SOS1
Big-M layout back to a first-class SOS1 constraint. The common transformation
contract is not restricted to lowering or to presolve reductions. The intended
next directions are:

- promotion from alternative regular or nonlinear formulations to higher-level
  constraint families beyond the currently recognized SOS1 Big-M layout;
- presolve transformations for MILP problems represented by `Instance`;
- more general presolve transformations beyond the current affine MILP
  fragment;
- an infeasibility-certificate framework that standardizes an appropriate
  relaxation, checks a certificate on the standardized problem, and translates
  the certified result back to the source `Instance`; and
- a canonical SDK-to-Lean bridge in which SDK-produced `Witness` values are
  checked against this formal contract.

The current in-Lean promotion `Witness` is untrusted layout data checked against
the source Instance. A future versioned SDK contract, bridge, and SDK-side
producer for such witnesses are intentionally unimplemented here.
