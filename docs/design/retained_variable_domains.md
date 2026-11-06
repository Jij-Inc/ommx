# Retaining declared variable domains

Status: proposed design for [#1360](https://github.com/Jij-Inc/ommx/issues/1360).
This PR proposes the contract and implementation slices; it does not implement
domain retention or change current evaluation behavior. Names used for proposed
records and queries below are descriptive, not commitments to SDK signatures.

## Example and decision

A user declares a continuous variable `x` in `[0, 10]` and a constraint
`x <= 5`. Bound tightening can send the solver an effective domain `[0, 5]`.
For a candidate `x = 7`, diagnostics should distinguish three facts:

| Check | Result in exact arithmetic |
| --- | --- |
| Declared domain `[0, 10]` | Satisfied |
| Effective domain `[0, 5]` | Violated |
| Source constraint `x <= 5` | Violated |

Passing the declared-domain check alone does not make the candidate source
feasible. Conversely, failing an inferred bound must not be reported as a
violation of the user's originally declared domain. Numerical decisions also
depend on the explicit tolerance and canonicalization policy used for evaluation.

Retain the declared domain when a solver-oriented operation first changes it.
Keep effective domains as the solver-facing data. Retention belongs to the
Interpretation Context held by the same `Instance`, rather than to an adapter
or a second complete copy of the problem. Domain recovery is independent of
whether an operation preserves feasibility or optimality.

## Current implementation

At the merge of #1363 (`c368fb917884f83aec968a7f5c17b76fc6894478`):

- `Instance::tighten_bounds_simultaneously_once_using_constraints` infers a
  batch of bounds, then calls `clip_bounds`. No pre-inference domain or
  inference provenance is retained.
- `DecisionVariableTable` owns the current variable rows, labels and fixed
  values. `DecisionVariable` owns the normalized kind/domain invariant.
- `Instance::evaluate` populates fixed and dependent values, evaluates retained
  expressions, and constructs evaluated variables from the current rows.
  `Solution::feasible_decision_variables` checks those current domains.
- v2 definition, evaluated and sampled variable tables do not represent a
  separate declared domain. v1 also has no such field.

The API Reference's existing warning about domain retention remains correct
until the implementation slices below land. This proposal does not redefine
`Solution.feasible` or imply that the original problem can already be recovered.

## Ownership and representation

`Instance` owns the distinction between explicit model edits and inference from
its active rows. The variable table can enforce local ID/domain consistency;
only `Instance` can associate an inference with constraints and transformations.

Store a sparse declared-domain record keyed by `VariableID`. Capture the domain
before the first solver-oriented change; subsequent passes must not overwrite
it. Absence means that no distinct declared domain has been recorded, and the
current domain serves both roles. An explicitly retained record remains present
even if later operations make the two domains equal.

The record must represent the full domain: kind plus interval or exact finite
set. An interval envelope is not a substitute for a finite domain, and a
semi-variable domain includes its zero alternative. Reuse the validated domain
representation without duplicating labels, fixed values, or the variable ID.
The first implementation need only infer bounds for the kinds supported by
today's tightening pass; unsupported kinds must not be silently coerced.

Each successful tightening batch also records:

- the operation and version of its numerical policy;
- changed variables and their entry/resulting domains;
- source row IDs and immutable source expressions/relations actually used;
- the input domains used by those inferences, including earlier inferred bounds;
- tolerance and selection settings needed to interpret the derivation.

Retaining only row IDs is insufficient: the corresponding rows may later be
relaxed or replaced. Retaining only the initial domain is sufficient for its
membership diagnostic, but is not evidence for an inference or its continued
validity. Provenance is an explanation of what was done, not a formal proof.

Publish effective domains, declared-domain captures, and batch provenance
atomically. Failed or unchanged passes add no records. Preserve the existing
bound inference algorithm and tolerance rules in the first implementation;
domain retention is not an opportunity to change them implicitly.

## Operation contracts

| Operation | Declared-domain and provenance behavior |
| --- | --- |
| Construction or explicit domain edit before inference | Establish the declared domain using the existing normalized domain rules. |
| Solver-oriented tightening | Capture once, narrow only the effective domain, and append an inference batch. |
| Repeated tightening | Preserve the first declared domain; record the new entry/result and dependency on earlier inferences. |
| Fixing or substitution | Preserve the source variable's declared domain and inference records while storing the existing fixed/dependent reconstruction rule. |
| Integer encoding | Keep the encoded source variable's domain context; generated bits have their own declared/effective binary domains. |
| Parameter specialization | Preserve domains; specialize parameter-bearing retained row snapshots together with the corresponding provenance. |
| Serialization and copying | Preserve record presence and meaning, including cases where declared and effective domains are equal. |

Explicit source-domain editing after inference is a different operation from
adding a further effective bound. Do not overload `clip_bounds` to guess intent.
The implementation must expose the two intentions explicitly and document the
migration of existing clipping calls, coordinated with #1229. Tightening must
use an owner-private commit path rather than invoking an API documented as an
explicit source edit.

For the first slice, source-domain edits after retained inference are rejected
atomically with guidance to edit the pre-tightening instance and run inference
again. This avoids silently keeping downstream bounds derived from a domain the
user has changed. It also avoids pretending that the recorded domain is still
the declaration after a user edit. Supporting in-place source edits and
recomputation later requires its own dependency invalidation contract.

An explicit effective-domain edit can keep historical provenance, but must not
acquire an inference guarantee simply because an earlier batch was valid.
Fixed-value consistency and the substituted variable's original kind remain
the responsibility of their existing domain owners. Source-domain retention
does not turn generic substitution into an equivalence-preserving operation.

## Relaxation, restoration, and inference validity

Removing or replacing a premise can invalidate an inference's justification.
For example, removing `x <= 5` after tightening to `[0, 5]` does not make the
effective domain `[0, 10]` again. A row-restoration operation likewise does not
restore the entire pre-tightening problem.

The first slice retains effective bounds and historical records on such edits,
marks the affected inference justification as invalidated, and transports no
source-equivalence guarantee through that invalidated batch. It must not label
the remaining bound as currently justified by the removed premise. Conservative
invalidation of all inference batches after a premise-changing edit is
acceptable initially; precise dependency tracking can follow later.

Users wanting a relaxation of the pre-tightening problem must start from that
instance and rerun the selected transformations. Automatically widening bounds
would require undoing dependent inferences, checking fixed/dependent variables,
and accounting for encodings or other operations that consumed those bounds.
That rollback is outside the first slice. Retaining declared domains alone does
not promise an inverse operation or a complete source-model snapshot.

## Evaluation and diagnostics

Expose declared-domain and effective-domain membership separately for each
variable. Evaluate both on the same populated state with an explicit tolerance
policy; report the policy with the result. Keep raw supplied values distinct
from canonicalized values when diagnostics explain a rounding decision. Do not
derive one domain-membership result from the other or use `atol` as an implicit
permission to relabel the effective domain as the declaration.

For fixed and dependent variables, use their authoritative reconstructed values
for declared-domain interpretation. For active solver variables, separately
check the effective solver domain. Preserve generated-variable diagnostics
without presenting generated variables as declarations from the original model.

Return both diagnostics even for an infeasible candidate when the existing
state-shape and numeric validation permits evaluation. Unknown IDs, missing
required values, and non-finite input still follow the existing error contract.
Apply the same rules to sample evaluation and per-sample extraction.

The first slice keeps current aggregate feasibility semantics and adds explicit
domain diagnostics. It must not silently substitute declared-domain membership
into `Solution.feasible`, `feasible_relaxed`, or `feasible_decision_variables`.
The broader query redesign belongs to #1217/#1218. A source-feasibility claim
requires the appropriate retained constraints and reconstruction as well as
declared domains; an optimality claim additionally requires a supported
transformation guarantee (#1364 and #1188).

## Serialization and validation

Use typed v2 data for declared domains and inference records. An annotation
string, modeling label, or fabricated removed regular constraint is not a domain
snapshot. An older reader must not silently discard these semantics: add a
required v2 feature for roots carrying them, following the existing feature
gate. Specify equivalent retention for evaluated and sampled results so saved
diagnostics are interpretable without access to a mutable source instance.

Parsing must validate existing variable IDs, domain normalization, record
presence, batch ordering, premise references/snapshots, and fixed/dependent
value consistency under the stated policy. An invalidated historical inference
is valid data, not permission to use a stale guarantee. Older payloads without
retention remain valid and mean that no distinct declaration was retained;
they must not invent a historical source domain.

v1 export must reject nonrepresentable retained semantics explicitly. It must
not silently flatten them into the effective domain. A future explicit lossy
projection may be considered separately. Feature detection, all root conversion
paths, schema generation, and round-trip coverage belong in the implementation.

## Implementation slices and acceptance evidence

The design can be reviewed independently; an implementation is complete only
after all applicable boundaries below are covered.

1. **Domain records and wire contract.** Add the root-owned sparse records,
   typed batch provenance, validation and feature-gated v2 encoding. Cover
   old-payload defaults, malformed records, unknown features, v1 rejection,
   and round trips through Instance and ParametricInstance. Use property tests
   as well as fixtures to protect cross-table invariants.
2. **Atomic capture and mutation contracts.** Integrate tightening, explicit
   source/effective edits, fixing, substitution and encoding. Include the
   no-change path, repeated passes, late arithmetic failure, and premise edits
   that invalidate provenance. Solver-used variables must continue to derive
   only from Active Formulation; domain/provenance-only references do not make
   a variable solver-used.
3. **Evaluation and persistence.** Add separate domain diagnostics for state
   and sample results, including serialization and sample extraction. Exercise
   the example above, discrete tolerance boundaries, semi and finite domains,
   fixed/dependent values, and generated variables. Preserve current aggregate
   feasibility behavior until its separate redesign is accepted.
4. **Public surfaces.** Expose coherent Rust/Python queries and explicit edit
   intentions, regenerate stubs, and document the supported workflow and
   migration in both languages. Replace the current API-reference limitation
   only when the corresponding behavior exists.

Related: #1361/#1362 establish the terminology; #1363 preserves reporting
expressions; #1359 retains source constraint rows; #1229 and #1180 own clipping
and tolerance policy; #1188 owns composed presolve guarantees. These are scoped
relationships, not a requirement to complete every roadmap before retaining
the first domain.
