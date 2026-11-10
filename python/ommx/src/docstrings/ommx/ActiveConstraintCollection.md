Live, read-only mapping of all active constraint families in a model.

Obtained from {attr}`~ommx.Instance.active_constraints` or
{attr}`~ommx.ParametricInstance.active_constraints`. The collection keeps its owner
alive and observes subsequent owner mutations. Obtaining it does not collect
constraint rows; indexing fetches the selected row from the owner's current
state. Active model rows are attached handles whose label setters update that
owner.

>>> from ommx import ConstraintID, Instance
>>> model = Instance.minimize()
>>> x = model.new_binary("x")
>>> rule = model.add_constraint(x <= 1, "limit")
>>> rows = model.active_constraints
>>> cid = rule.constraint_id
>>> rows[cid].set_name("capacity")
>>> assert model.active_constraints[cid].name == "capacity"
>>> assert rows.get(ConstraintID(99)) is None
>>> assert rows.get(ConstraintID(99), "missing") == "missing"

**Typed keys and row ownership**

Mixed-family lookups require typed IDs, even though constraint numbers are
unique across all families in an Instance. Bare integers raise ``TypeError``;
a missing typed ID raises ``KeyError``. ``get(id)`` returns ``None`` for a
missing typed ID, or the supplied default in ``get(id, default)``. Indexing and
``get`` preserve the concrete row type for a known concrete ID; the latter's
return type also includes the default's type. See {class}`~ommx.ConstraintIDBase`
for ID families, integer promotion, and the extension contract.

The collection cannot be changed by item assignment or deletion. Change the
model through its owner methods. Related collections use the same lookup and
iteration rules but have different row ownership:

| Collection | Included constraints | Returned rows |
| --- | --- | --- |
| {class}`~ommx.ActiveConstraintCollection` | Active model constraints | Attached handles |
| {class}`~ommx.InactiveConstraintCollection` | Retained inactive model constraints | Independent snapshots |
| {class}`~ommx.EvaluatedConstraintCollection` | Active and retained inactive constraints in a Solution | Independent evaluated snapshots |
| {class}`~ommx.SampledConstraintCollection` | Active and retained inactive constraints in a SampleSet | Independent sampled snapshots |

Attached handles compare equal when they refer to the same owner object and
typed ID. Handles from different owners are unequal even if the models contain
identical data. Inactive and result snapshots compare their stored data, including
metadata and inactivity reasons. Mapping equality and membership in ``values()`` or
``items()`` use those row equality rules.

**Live views and iteration**

``keys()``, ``values()``, and ``items()`` are live views. ``iter(rows)`` snapshots
typed IDs immediately; iterators over the standard Python views snapshot them on
the first ``next()``. Within the currently supported families, IDs follow family
order (regular, indicator, one-hot, SOS1), then numeric order. Values are fetched
as they are visited. If a snapshotted key is removed before its value is fetched,
that access raises ``KeyError``.

>>> ids_before_addition = iter(rows)
>>> keys = rows.keys()
>>> added = model.add_constraint(x >= 0, "lower")
>>> assert list(ids_before_addition) == [cid]
>>> assert list(keys) == [cid, added.constraint_id]
>>> assert rows == dict(rows)

``dict(rows)`` materializes a mapping at that time; its active model values are
still attached handles. To inspect just one family, use ``of_kind`` and
{class}`~ommx.ConstraintFamilyView`. To collect a table, use the owner's
``constraints_df(kind=..., min_activeness=ConstraintActiveness.ACTIVE)``. Its typed
index matches this collection's keys for the selected family. The default
``min_activeness=ConstraintActiveness.INACTIVE_SKIPPED`` includes all active and
inactive rows, including those skipped by default feasibility checks.

```pycon
>>> from ommx import ConstraintActiveness, ConstraintKind
>>> model.deactivate_constraint(cid, "retain for reporting")
>>> table = model.constraints_df(
...     kind=ConstraintKind.Regular,
...     min_activeness=ConstraintActiveness.ACTIVE,
... )
>>> assert set(table.index) == set(rows.of_kind(ConstraintKind.Regular))
>>> assert cid not in table.index
>>> assert cid in model.constraints_df(kind=ConstraintKind.Regular).index

```
