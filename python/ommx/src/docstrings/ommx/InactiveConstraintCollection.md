Live mapping of all inactive constraint families in a model.

Obtained from {attr}`~ommx.Instance.inactive_constraints` or
{attr}`~ommx.ParametricInstance.inactive_constraints`. Each access returns an
independent snapshot, including labels, provenance, and inactivity reasons.
Changing that snapshot does not update its owner; a later lookup reads the
owner's current state.

Inactive constraints are excluded from the active formulation but still checked
when candidates are evaluated. Deactivation can follow relaxation, an equivalent
lowering, or another transformation; the collection's presence does not identify
which guarantee holds. See {attr}`~ommx.Instance.inactive_constraints` for their
evaluation semantics and {meth}`~ommx.Instance.deactivate_constraint` for a complete
relax, inspect, evaluate, and restore example.

The collection keeps its owner alive. The typed-key, ``get``, live-view,
iteration, and snapshot equality rules are documented in
{class}`~ommx.ActiveConstraintCollection`. Use ``of_kind`` and
{class}`~ommx.ConstraintFamilyView` to inspect one family. A model's
``constraints_df(kind=..., min_activeness=ConstraintActiveness.SKIP_FEASIBILITY_CHECK)`` includes both active and inactive rows.
