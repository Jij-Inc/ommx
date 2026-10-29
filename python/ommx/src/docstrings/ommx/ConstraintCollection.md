Live, read-only mapping of all active constraint families in a model.

Obtained from {attr}`~ommx.Instance.constraints` or
{attr}`~ommx.ParametricInstance.constraints`. The collection keeps its owner
alive and observes subsequent owner mutations. Obtaining it does not collect
constraint rows; indexing fetches the selected row from the owner's current
state. Active model rows are attached handles whose label setters update that
owner.

>>> from ommx import ConstraintID, Instance
>>> model = Instance.minimize()
>>> x = model.new_binary("x")
>>> rule = model.add_constraint(x <= 1, "limit")
>>> rows = model.constraints
>>> cid = rule.constraint_id
>>> rows[cid].set_name("capacity")
>>> assert model.constraints[cid].name == "capacity"
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
| {class}`~ommx.ConstraintCollection` | Active model constraints | Attached handles |
| {class}`~ommx.RemovedConstraintCollection` | Retained removed model constraints | Independent snapshots |
| {class}`~ommx.EvaluatedConstraintCollection` | Active and retained removed constraints in a Solution | Independent evaluated snapshots |
| {class}`~ommx.SampledConstraintCollection` | Active and retained removed constraints in a SampleSet | Independent sampled snapshots |

Attached handles compare equal when they refer to the same owner object and
typed ID. Handles from different owners are unequal even if the models contain
identical data. Removed and result snapshots compare their stored data, including
metadata and removal reasons. Mapping equality and membership in ``values()`` or
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
``constraints_df(kind=...)``. Its typed index matches the selected family's keys;
on models, ``removed=True`` includes both active and retained removed rows.
