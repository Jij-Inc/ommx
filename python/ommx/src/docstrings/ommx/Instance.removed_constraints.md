Live mapping of retained constraints that are no longer active.

Active constraints in {attr}`~ommx.Instance.constraints` define the current
solver-facing formulation. Removed constraints remain available for evaluation
and inspection, but are not imposed as active conditions. Both collections
include regular constraints and supported special-constraint families, currently
Indicator, OneHot, and SOS1, keyed by their typed IDs.

The returned {class}`~ommx.RemovedConstraintCollection` keeps this Instance alive
and observes subsequent owner changes. Each lookup returns an independent
snapshot with labels, provenance, and the removal reason; editing a snapshot
does not change the Instance. See that class for lookup and iteration semantics.

**Removal records a change, not its mathematical guarantee**

Different operations retain removed constraints for different reasons:

| Operation | Active formulation | Retained information |
| --- | --- | --- |
| {meth}`~ommx.Instance.relax_constraint` or {meth}`~ommx.Instance.relax_indicator_constraint` | Omits the selected condition | The condition and caller-supplied removal reason |
| Special-constraint lowering, such as {meth}`~ommx.Instance.convert_one_hot_to_constraint` | Represents the condition using regular constraints | The original special constraint; generated constraints carry provenance |
| {meth}`~ommx.Instance.penalty_method` | Replaces regular constraints with weighted objective terms | The constraints to check independently of the penalized objective |
| Removal of trivial constraints during preprocessing | Omits conditions found unnecessary by that operation | The removed conditions and their reasons |

The presence of removed constraints alone therefore does not establish
equivalence, relaxation, or preservation of optimal solutions. Read the owning
transformation's applicability and guarantee; in particular, a finite penalty
weight does not by itself guarantee that the selected candidate satisfies the
removed constraints.

**Evaluation and inspection**

{meth}`~ommx.Instance.evaluate` and {meth}`~ommx.Instance.evaluate_samples`
evaluate both active and removed constraints after restoring and completing
variable values through {meth}`~ommx.Instance.populate_state`.
{attr}`~ommx.Solution.feasible` includes all retained constraints;
{attr}`~ommx.Solution.feasible_relaxed` excludes removed constraints. Both also
check every retained variable's kind and bounds. The corresponding
{attr}`~ommx.SampleSet.feasible` and {attr}`~ommx.SampleSet.feasible_relaxed`
properties return these judgments by sample ID.

A variable used only in removed constraints may be irrelevant to the active
formulation. If omitted, its default is used for evaluation. Supply its actual
value when checking a particular candidate; the default is not a recovered
solver choice.

For a table of active and removed rows, use
{meth}`~ommx.Instance.constraints_df` with `removed=True` and an explicit
`kind`. On a Solution, use {meth}`~ommx.Solution.constraints_df` with
`include=("removed_reason",)` to inspect removal reasons alongside evaluated
values and violations. The complete relax, evaluate, inspect, and restore
example is in {meth}`~ommx.Instance.relax_constraint`.
