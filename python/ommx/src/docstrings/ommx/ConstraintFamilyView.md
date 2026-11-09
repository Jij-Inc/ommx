Live, read-only mapping of one constraint family, created by ``of_kind``.

The selected family fixes the key type. Indexing, ``get``, and membership accept
integers as well as that family's typed IDs; an ID of another family raises
``TypeError``. Iteration and dictionary keys retain typed IDs. A known
``ConstraintKind`` constant fixes the key type. Filtered active/inactive and result
collections also preserve the concrete row type. The all-state model ``constraints``
view returns ``ConstraintView`` because a family ID does not encode activeness.

>>> from ommx import ConstraintKind, ConstraintID, Instance
>>> model = Instance.minimize()
>>> x = model.new_binary("x")
>>> rule = model.add_constraint(x <= 1, "limit")
>>> regular = model.active_constraints.of_kind(ConstraintKind.Regular)
>>> assert rule.constraint_id == ConstraintID(0)
>>> assert regular[0] == regular[ConstraintID(0)]
>>> assert list(regular) == [ConstraintID(0)]
>>> assert 0 in regular
>>> assert regular.get(99) is None
>>> regular[0].set_name("capacity")
>>> assert model.active_constraints[ConstraintID(0)].name == "capacity"

Pass a {class}`~ommx.ConstraintKind` selector, as in ``constraints_df(kind=...)``.
A string such as ``"sos1"`` raises ``TypeError``. See
{class}`~ommx.EvaluatedConstraintCollection` for the current ID and result-type table.

The view retains its owner without collecting rows. It follows the row ownership,
equality, and iterator snapshot rules of {class}`~ommx.ActiveConstraintCollection`:
active model rows are attached handles, while removed and result rows are
independent snapshots. This applies to Instance and ParametricInstance active
and removed collections, and to Solution and SampleSet result collections.

Filtering is an inspection operation. Selecting one family does not establish
that a solver or transformation supports every constraint in the model. A
consumer must inspect all constraints and reject unsupported families; see
{class}`~ommx.ConstraintIDBase`.
