Move an active regular constraint to the retained removed collection.

The Instance is changed in place. `constraint_id` accepts a {class}`~ommx.ConstraintID`
or its integer value. `reason` records why the condition was removed, and
additional keyword arguments record string-valued reason parameters. The
constraint keeps its ID, labels, and provenance.

Removed constraints no longer restrict the active formulation, but
{meth}`~ommx.Instance.evaluate` still checks them. Here the relaxed formulation
allows `x=1`, while the retained `x <= 0` condition rejects it:

```pycon
>>> from ommx import ConstraintKind, Instance
>>> instance = Instance.maximize()
>>> x = instance.new_binary("x")
>>> instance.objective = x
>>> limit = instance.add_constraint(x <= 0, "limit")
>>> limit_id = limit.constraint_id
>>> instance.relax_constraint(limit_id, "example.relax_limit", purpose="inspection")
>>> assert limit_id not in instance.constraints
>>> removed = instance.removed_constraints[limit_id]
>>> assert removed.removed_reason == "example.relax_limit"
>>> assert removed.removed_reason_parameters == {"purpose": "inspection"}
>>> solution = instance.evaluate({x.id: 1})
>>> assert solution.feasible_relaxed
>>> assert not solution.feasible

```

Inspect the removal reason on the model and the evaluated violation on the
result. `kind=ConstraintKind.Regular` selects the regular-constraint family; the same table
APIs also accept `"indicator"`, `"one_hot"`, and `"sos1"` for those families.

```pycon
>>> model_rows = instance.constraints_df(kind=ConstraintKind.Regular, removed=True)
>>> assert model_rows.loc[limit_id, "removed_reason"] == "example.relax_limit"
>>> result_rows = solution.constraints_df(
...     kind=ConstraintKind.Regular, include=("removed_reason",)
... )
>>> assert result_rows.loc[limit_id, "removed_reason"] == "example.relax_limit"
>>> assert result_rows.loc[limit_id, "removed_reason.purpose"] == "inspection"
>>> assert result_rows.loc[limit_id, "violation"] == 1.0
>>> instance.restore_constraint(limit_id)
>>> assert limit_id in instance.constraints
>>> assert limit_id not in instance.removed_constraints
>>> assert not instance.evaluate({x.id: 1}).feasible_relaxed

```

{meth}`~ommx.Instance.restore_constraint` makes the retained constraint active
again, using the Instance's current fixed values and variable dependencies.
It does not undo other transformations, remove generated variables or
constraints, or revert objective changes. The already-created Solution above
continues to describe its earlier evaluation.

Raises `RuntimeError` if the ID does not identify an active regular constraint.
Use {meth}`~ommx.Instance.relax_indicator_constraint` for an Indicator constraint.
See {attr}`~ommx.Instance.removed_constraints` for other operations that retain
removed constraints and for the distinction between removal and relaxation.
