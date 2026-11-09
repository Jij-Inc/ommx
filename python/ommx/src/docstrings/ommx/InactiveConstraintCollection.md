Live mapping of all retained inactive constraint families in a model.

Obtained from {attr}`~ommx.Instance.inactive_constraints` or
{attr}`~ommx.ParametricInstance.inactive_constraints`. Each access returns an
independent snapshot, including labels, provenance, and deactivation reasons.
Changing that snapshot does not update its owner; a later lookup reads the
owner's current state.

Inactive constraints are excluded from the active formulation but still checked
when candidates are evaluated. Deactivation can follow relaxation, an equivalent
lowering, or another transformation; the collection's presence does not identify
which guarantee holds. See {attr}`~ommx.Instance.inactive_constraints` for their
evaluation semantics and {meth}`~ommx.Instance.deactivate_constraint` for a complete
deactivate, inspect, evaluate, and activate example.

The collection keeps its owner alive. The typed-key, ``get``, live-view,
iteration, and snapshot equality rules are documented in
{class}`~ommx.ConstraintCollection`. Use ``of_kind`` and
{class}`~ommx.ConstraintFamilyView` to inspect one family. A model's
``constraints_df(kind=..., removed=True)`` includes both active and inactive rows.
