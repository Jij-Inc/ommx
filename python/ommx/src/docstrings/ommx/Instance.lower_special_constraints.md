Lower selected active special constraint kinds into regular constraints.

For every kind in ``kinds_to_lower``, the corresponding bulk conversion
is invoked
({meth}`~ommx.Instance.convert_all_indicators_to_constraints`,
{meth}`~ommx.Instance.convert_all_one_hots_to_constraints`, or
{meth}`~ommx.Instance.convert_all_sos1_to_constraints`) when that kind is active. The
instance is mutated in place. Kinds omitted from ``kinds_to_lower``
remain active, and an empty set is a no-op. This does not establish
{class}`~ommx.InstanceClass` membership; check the resulting input separately.

Returns the set of {class}`~ommx.ConstraintKind` values that were
requested and active, and therefore actually lowered. Empty when no
requested kind was active.

``atol`` controls zero-sensitive interval bounds used while lowering
Indicator constraints. If omitted, the default from {func}`~ommx.get_default_atol`
is used. This aligns zero-sensitive Function body evaluation; lowering itself assumes
exact discrete variable values.

Kinds are processed in ``Indicator``, ``OneHot``, ``Sos1`` order. The
Indicator and SOS1 bulk conversions validate their complete families
before mutation, including generated-ID capacity. OneHot checks ID
capacity for the whole family before converting in ascending original
ID order; other conversion failures may leave earlier OneHot conversions
committed. The whole operation is not transactional across
families: an error does not roll back earlier completed families.

Raises if an underlying conversion fails, including insufficient fresh
IDs or a required non-finite Big-M bound.

**Select a family and inspect the generated constraints**

This model contains OneHot and SOS1 constraints. Lowering only OneHot preserves
the active SOS1, retains the original OneHot for evaluation, and adds an equality.

```pycon
>>> from ommx import (
...     ConstraintID,
...     Instance,
...     OneHotConstraint,
...     OneHotConstraintID,
...     ProvenanceKind,
...     Sos1Constraint,
...     ConstraintKind,
... )
>>> instance = Instance.maximize()
>>> choices = [instance.new_binary("choice", subscripts=[i]) for i in range(3)]
>>> instance.objective = sum(choices)
>>> original = instance.add_one_hot_constraint(OneHotConstraint(variables=choices))
>>> _ = instance.add_sos1_constraint(Sos1Constraint(variables=choices))
>>> assert instance.active_special_constraint_kinds == {
...     ConstraintKind.OneHot,
...     ConstraintKind.Sos1,
... }
>>> assert instance.lower_special_constraints({ConstraintKind.OneHot}) == {
...     ConstraintKind.OneHot,
... }
>>> assert instance.active_special_constraint_kinds == {ConstraintKind.Sos1}
>>> assert original.constraint_id not in instance.constraints
>>> removed = instance.removed_constraints[original.constraint_id]
>>> assert removed.removed_reason == "ommx.Instance.convert_one_hot_to_constraint"
>>> assert removed.removed_reason_parameters == {"constraint_id": "2"}
>>> generated = instance.constraints[ConstraintID(2)]
>>> assert generated.provenance[-1].kind == ProvenanceKind.OneHotConstraint
>>> assert generated.provenance[-1].original_id == OneHotConstraintID(0)
>>> rows = instance.constraints_df(kind=ConstraintKind.OneHot, removed=True)
>>> assert rows.loc[original.constraint_id, "removed_reason"] == removed.removed_reason
>>> assert instance.evaluate(
...     {choices[0].id: 1, choices[1].id: 0, choices[2].id: 0}
... ).feasible
>>> assert instance.lower_special_constraints({ConstraintKind.OneHot}) == set()
>>> assert instance.lower_special_constraints(set()) == set()

```

`removed=True` includes active and removed rows for the selected family and adds
the removal-reason columns. Each generated regular constraint records its source
through {attr}`~ommx.Constraint.provenance`; removed rows record generated IDs in
their reason parameters. The single-constraint conversion APIs define the
family-specific parameter names and ordering. See
{meth}`~ommx.Instance.convert_one_hot_to_constraint`,
{meth}`~ommx.Instance.convert_sos1_to_constraints`, and
{meth}`~ommx.Instance.convert_indicator_to_constraint` for their formulas,
preconditions, complete examples, and per-operation mutation guarantees.

After evaluation, {meth}`~ommx.Solution.constraints_df` accepts
`include=("removed_reason",)` to show those reasons alongside violations. The
same family selection is available for {class}`~ommx.SampleSet`.
