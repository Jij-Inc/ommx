Live mapping of a SampleSet's constraint results, including retained removed rows.

Obtained from {attr}`~ommx.SampleSet.constraints`. Each access returns an
independent snapshot with labels, provenance, removal reasons, and sampled
evaluation diagnostics. Changing that snapshot does not update the SampleSet;
a later lookup reads the owner's current state. The collection keeps its owner
alive.

The typed-key, ``get``, live-view, iteration, and snapshot equality rules are
documented in {class}`~ommx.ConstraintCollection`. See
{class}`~ommx.EvaluatedConstraintCollection` for the current ID and result-type
table, and {class}`~ommx.ConstraintFamilyView` for ``of_kind`` filtering.
Sampled rows expose ``violations`` and ``feasible(atol=...)`` keyed by sample ID.
Use {attr}`~ommx.SampleSet.summary` for per-sample objectives and overall
feasibility, or {meth}`~ommx.SampleSet.constraints_df` with an explicit ``kind``
for a table of one constraint family, including retained removed rows.
