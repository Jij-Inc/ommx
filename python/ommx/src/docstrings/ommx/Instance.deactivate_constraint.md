Move an active regular constraint to the inactive collection.

The Instance is changed in place. `constraint_id` accepts a {class}`~ommx.ConstraintID`
or its integer value. `reason` records why the condition was deactivated, and
additional keyword arguments record string-valued reason parameters. The
constraint keeps its ID, labels, and provenance.

Set `skip_feasibility_check=True` to exclude it from default aggregate
feasibility and total-violation queries. The row is still evaluated; explicit
queries with the minimum `SKIP_FEASIBILITY_CHECK` include it.

Inactive constraints no longer restrict the active formulation, but
{meth}`~ommx.Instance.evaluate` still checks them. Here the relaxed formulation
allows `x=1`, while the retained `x <= 0` condition rejects it:

```pycon
>>> from ommx import ConstraintKind, Instance
>>> instance = Instance.maximize()
>>> x = instance.new_binary("x")
>>> instance.objective = x
>>> limit = instance.add_constraint(x <= 0, "limit")
>>> limit_id = limit.constraint_id
>>> instance.deactivate_constraint(limit_id, "example.relax_limit", purpose="inspection")
>>> assert limit_id not in instance.active_constraints
>>> inactive = instance.inactive_constraints[limit_id]
>>> assert inactive.inactive_reason == "example.relax_limit"
>>> assert inactive.inactive_reason_parameters == {"purpose": "inspection"}
>>> solution = instance.evaluate({x.id: 1})
>>> assert solution.feasible_relaxed
>>> assert not solution.feasible()

```

Inspect the inactivity reason on the model and the evaluated violation on the
result. Use `kind=ConstraintKind.Regular` for regular constraints. The same table
APIs accept `ConstraintKind.Indicator`, `ConstraintKind.OneHot`, and `ConstraintKind.Sos1`
for the corresponding special families.

```pycon
>>> model_rows = instance.constraints_df(
...     kind=ConstraintKind.Regular, include=("inactive_reason",)
... )
>>> assert model_rows.loc[limit_id, "inactive_reason"] == "example.relax_limit"
>>> result_rows = solution.constraints_df(
...     kind=ConstraintKind.Regular, include=("inactive_reason",)
... )
>>> assert result_rows.loc[limit_id, "inactive_reason"] == "example.relax_limit"
>>> assert result_rows.loc[limit_id, "inactive_reason.purpose"] == "inspection"
>>> assert result_rows.loc[limit_id, "violation"] == 1.0
>>> instance.activate_constraint(limit_id)
>>> assert limit_id in instance.constraints
>>> assert limit_id not in instance.inactive_constraints
>>> assert not instance.evaluate({x.id: 1}).feasible_relaxed

```

{meth}`~ommx.Instance.activate_constraint` makes the retained constraint active
again, using the Instance's current fixed values and variable dependencies.
It does not undo other transformations, remove generated variables or
constraints, or revert objective changes. The already-created Solution above
continues to describe its earlier evaluation.

Raises `RuntimeError` if the ID does not identify an active regular constraint.
Use {meth}`~ommx.Instance.deactivate_indicator_constraint` for an Indicator constraint.
See {attr}`~ommx.Instance.inactive_constraints` for other operations that retain
inactive constraints and for the distinction between deactivation and relaxation.
