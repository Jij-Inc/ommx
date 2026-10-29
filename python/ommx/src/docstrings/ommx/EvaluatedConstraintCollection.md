Live mapping of a Solution's constraint results, including retained removed rows.

Obtained from {attr}`~ommx.Solution.constraints`. The collection keeps its
Solution alive and fetches each row on access. Each row is an independent
snapshot containing labels, provenance, removal reasons, and evaluation
diagnostics. Editing a fetched snapshot does not update the Solution. Use an
owner method such as {meth}`~ommx.Solution.set_dual_variable` to update the result;
subsequent collection access observes that update.

>>> from ommx import Instance
>>> model = Instance.minimize()
>>> x = model.new_binary("x")
>>> upper = model.add_constraint(x <= 1, "upper")
>>> lower = model.add_constraint(x >= 0, "lower")
>>> model.relax_constraint(lower.constraint_id, "inspect later")
>>> solution = model.evaluate({x.id: 0})
>>> rows = solution.constraints
>>> assert lower.constraint_id in rows
>>> assert rows[lower.constraint_id].removed_reason == "inspect later"
>>> snapshot = rows[upper.constraint_id]
>>> solution.set_dual_variable(upper.constraint_id, 3.0)
>>> assert snapshot.dual_variable is None
>>> assert rows[upper.constraint_id].dual_variable == 3.0
>>> snapshot.dual_variable = 8.0
>>> assert rows[upper.constraint_id].dual_variable == 3.0

**Result types**

The currently supported families have the following concrete result types.
The set of possible families is open; use {class}`~ommx.ConstraintIDBase` and
{class}`~ommx.ConstraintView` for common interfaces, and explicitly reject
unsupported families when interpreting their mathematical meaning.

| ``kind`` | Typed ID | Solution row | SampleSet row |
| --- | --- | --- | --- |
| ``"regular"`` | {class}`~ommx.ConstraintID` | {class}`~ommx.EvaluatedConstraint` | {class}`~ommx.SampledConstraint` |
| ``"indicator"`` | {class}`~ommx.IndicatorConstraintID` | {class}`~ommx.EvaluatedIndicatorConstraint` | {class}`~ommx.SampledIndicatorConstraint` |
| ``"one_hot"`` | {class}`~ommx.OneHotConstraintID` | {class}`~ommx.EvaluatedOneHotConstraint` | {class}`~ommx.SampledOneHotConstraint` |
| ``"sos1"`` | {class}`~ommx.Sos1ConstraintID` | {class}`~ommx.EvaluatedSos1Constraint` | {class}`~ommx.SampledSos1Constraint` |

For the mixed-family typed-key contract, ``get``, live views, iteration timing,
and snapshot equality, see {class}`~ommx.ConstraintCollection`. Filter without
collecting rows using ``of_kind`` and {class}`~ommx.ConstraintFamilyView`.
{meth}`~ommx.Solution.constraints_df` collects one family as a DataFrame;
retained removed constraints remain included. Its ``kind`` argument is required,
as it is for ``constraint_context_df``, ``constraint_parameters_df``,
``constraint_provenance_df``, and ``constraint_removed_reasons_df`` on all owners.

For multiple candidates, {class}`~ommx.SampledConstraintCollection` returns
sampled rows. Their ``violations`` and ``feasible(atol=...)`` are keyed by sample
ID. Per-sample objective and overall feasibility belong to
{attr}`~ommx.SampleSet.summary`, rather than the per-constraint rows.
