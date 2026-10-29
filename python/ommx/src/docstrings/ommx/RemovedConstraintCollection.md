Live mapping of all retained removed constraint families in a model.

Obtained from {attr}`~ommx.Instance.removed_constraints` or
{attr}`~ommx.ParametricInstance.removed_constraints`. Each access returns an
independent snapshot, including labels, provenance, and removal reasons.
Changing that snapshot does not update its owner; a later lookup reads the
owner's current state.

The collection keeps its owner alive. The typed-key, ``get``, live-view,
iteration, and snapshot equality rules are documented in
{class}`~ommx.ConstraintCollection`. Use ``of_kind`` and
{class}`~ommx.ConstraintFamilyView` to inspect one family. A model's
``constraints_df(kind=..., removed=True)`` includes both active and removed rows.
